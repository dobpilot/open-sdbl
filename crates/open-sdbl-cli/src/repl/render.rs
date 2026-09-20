//! Rendering a result as a table: column widths, row shaping and the
//! terminal-aware layout.

use std::borrow::Cow;
use std::io::{self, IsTerminal, Write};

use open_sdbl::metadata::MetadataSnapshot;
use open_sdbl::query::{ColumnKind, CompiledColumn, CompiledQuery, TypeValue};
use open_sdbl_db::{Cell, QueryRows};
use unicode_width::UnicodeWidthStr;

use crate::error::CliError;
use crate::output::{MAX_CELL_WIDTH, MAX_PRINTED_ROWS, bounded_field, escape_field};

use super::*;

#[cfg(test)]
#[path = "../tests/repl_render.rs"]
mod tests;

pub(super) fn validate_query_rows(
    compiled: &CompiledQuery,
    rows: &QueryRows,
) -> Result<(), CliError> {
    for row in rows {
        if row.len() != compiled.columns.len() {
            return Err(CliError::Data(format!(
                "database returned {} columns, expected {}",
                row.len(),
                compiled.columns.len()
            )));
        }
    }
    Ok(())
}

pub(super) fn print_query_rows(
    output: &mut impl Write,
    snapshot: &MetadataSnapshot,
    compiled: &CompiledQuery,
    rows: &QueryRows,
) -> io::Result<()> {
    print_result_table(
        output,
        snapshot,
        &compiled.columns,
        &compiled.service_columns,
        rows,
    )
}

/// Prints one result table, leaving out the columns the compiler carries
/// for itself — the owner key a nested result links to.
pub(super) fn print_result_table(
    output: &mut impl Write,
    snapshot: &MetadataSnapshot,
    columns: &[CompiledColumn],
    service: &[usize],
    rows: &QueryRows,
) -> io::Result<()> {
    let shown: Vec<usize> = (0..columns.len())
        .filter(|index| !service.contains(index))
        .collect();
    let headers: Vec<&str> = shown
        .iter()
        .map(|index| columns[*index].label.as_str())
        .collect();
    let kinds: Vec<&ColumnKind> = shown.iter().map(|index| &columns[*index].kind).collect();
    let visible: Vec<Vec<Cell>> = rows
        .iter()
        .map(|cells| {
            shown
                .iter()
                .map(|index| cells[*index].clone())
                .collect::<Vec<_>>()
        })
        .collect();
    let typed: Vec<TypedRow<'_>> = visible
        .iter()
        .map(|cells| TypedRow {
            cells,
            kinds: &kinds,
            snapshot,
        })
        .collect();
    print_table(output, &headers, &typed)?;
    writeln!(output, "({} rows)", rows.len())
}

/// One result row rendered with its column kinds, so a type value prints
/// as the name of the type rather than as its five bytes.
pub(super) struct TypedRow<'a> {
    cells: &'a Vec<Cell>,
    kinds: &'a [&'a ColumnKind],
    snapshot: &'a MetadataSnapshot,
}

impl TableRow for TypedRow<'_> {
    fn cell(&self, index: usize) -> Cow<'_, str> {
        let Some(cell) = self.cells.get(index) else {
            return Cow::Borrowed("");
        };
        if self
            .kinds
            .get(index)
            .is_some_and(|kind| **kind == ColumnKind::Type)
            && let Some(value) = cell.as_bytes().and_then(TypeValue::decode)
        {
            return Cow::Owned(value.query_name(self.snapshot));
        }
        cell.render()
    }
}

pub(super) trait TableRow {
    fn cell(&self, index: usize) -> Cow<'_, str>;
}

impl TableRow for Vec<String> {
    fn cell(&self, index: usize) -> Cow<'_, str> {
        Cow::Borrowed(self.get(index).map_or("", String::as_str))
    }
}

impl TableRow for Vec<Cell> {
    fn cell(&self, index: usize) -> Cow<'_, str> {
        self.get(index).map_or(Cow::Borrowed(""), Cell::render)
    }
}

pub(super) struct HeaderRow<'a>(&'a [&'a str]);

impl TableRow for HeaderRow<'_> {
    fn cell(&self, index: usize) -> Cow<'_, str> {
        Cow::Borrowed(self.0.get(index).copied().unwrap_or(""))
    }
}

pub(super) fn print_table<R: TableRow>(
    output: &mut impl Write,
    headers: &[&str],
    rows: &[R],
) -> io::Result<()> {
    print_table_with_width(output, headers, rows, detected_table_width())
}

pub(super) fn print_table_with_width<R: TableRow>(
    output: &mut impl Write,
    headers: &[&str],
    rows: &[R],
    terminal_width: Option<usize>,
) -> io::Result<()> {
    let mut widths = headers
        .iter()
        .map(|header| display_width(header).clamp(1, MAX_CELL_WIDTH))
        .collect::<Vec<_>>();
    for row in rows.iter().take(MAX_PRINTED_ROWS) {
        for (index, width) in widths.iter_mut().enumerate() {
            *width = (*width)
                .max(display_width(&row.cell(index)))
                .min(MAX_CELL_WIDTH);
        }
    }
    let omitted_columns = terminal_width.map_or(0, |terminal_width| {
        fit_table_widths(&mut widths, terminal_width)
    });
    if widths.is_empty() {
        if omitted_columns != 0 {
            writeln!(output, "({omitted_columns} columns omitted)")?;
        }
        return Ok(());
    }

    write_table_row(output, &HeaderRow(headers), &widths)?;
    for (index, width) in widths.iter().enumerate() {
        if index != 0 {
            output.write_all(b"-+-")?;
        }
        output.write_all("-".repeat(*width).as_bytes())?;
    }
    writeln!(output)?;
    for row in rows.iter().take(MAX_PRINTED_ROWS) {
        write_table_row(output, row, &widths)?;
    }
    if omitted_columns != 0 {
        writeln!(output, "({omitted_columns} columns omitted)")?;
    }
    let omitted = rows.len().saturating_sub(MAX_PRINTED_ROWS);
    if omitted != 0 {
        writeln!(output, "({omitted} rows omitted)")?;
    }
    Ok(())
}

pub(super) fn fit_table_widths(widths: &mut Vec<usize>, terminal_width: usize) -> usize {
    let original_columns = widths.len();
    if terminal_width == 0 {
        widths.clear();
        return original_columns;
    }
    while widths.len() > 1 && widths.len() * 4 - 3 > terminal_width {
        widths.pop();
    }
    let separators = widths.len().saturating_sub(1) * 3;
    let available = terminal_width.saturating_sub(separators);
    while widths.iter().sum::<usize>() > available {
        let Some((index, _)) = widths
            .iter()
            .enumerate()
            .filter(|(_, width)| **width > 1)
            .max_by_key(|(_, width)| **width)
        else {
            break;
        };
        widths[index] -= 1;
    }
    original_columns - widths.len()
}

pub(super) fn write_table_row(
    output: &mut impl Write,
    values: &impl TableRow,
    widths: &[usize],
) -> io::Result<()> {
    for (index, width) in widths.iter().enumerate() {
        if index != 0 {
            output.write_all(b" | ")?;
        }
        let value = bounded_field(&values.cell(index), *width);
        let padding = width.saturating_sub(UnicodeWidthStr::width(value.as_str()));
        output.write_all(value.as_bytes())?;
        output.write_all(" ".repeat(padding).as_bytes())?;
    }
    writeln!(output)
}

pub(super) fn display_width(value: &str) -> usize {
    UnicodeWidthStr::width(escape_field(value).as_str())
}

pub(super) fn detected_table_width() -> Option<usize> {
    if !io::stdout().is_terminal() {
        return None;
    }
    #[cfg(target_os = "linux")]
    {
        terminal_size().map(|(_, columns)| usize::from(columns))
    }
    #[cfg(not(target_os = "linux"))]
    {
        std::env::var("COLUMNS")
            .ok()
            .and_then(|columns| columns.parse().ok())
    }
}
