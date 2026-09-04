use std::fmt::Write as _;
use std::fs;
use std::io::{self, Read, Write};

use open_sdbl::metadata::{MetadataSnapshot, ResolutionReport};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::error::CliError;

pub(crate) const MAX_PRINTED_ROWS: usize = 1_000;
pub(crate) const MAX_CELL_WIDTH: usize = 256;

pub(crate) fn print_resolution_report(report: &ResolutionReport) {
    const MAX_PRINTED_FINDINGS: usize = 100;
    for finding in report.findings().iter().take(MAX_PRINTED_FINDINGS) {
        eprintln!(
            "metadata resolution: {}",
            escape_field(&finding.to_string())
        );
    }
    let omitted = report.findings().len().saturating_sub(MAX_PRINTED_FINDINGS);
    if omitted != 0 {
        eprintln!("metadata resolution: {omitted} additional findings omitted");
    }
}

pub(crate) fn print_snapshot(
    output: &mut impl Write,
    snapshot: &MetadataSnapshot,
) -> io::Result<()> {
    writeln!(
        output,
        "RECORD\tGUID\tKIND\tNAME\tPHYSICAL_NAME\tOWNER\tSCHEMA\tLIVE\tDETAIL"
    )?;
    let total_rows = snapshot.objects().len() + snapshot.fields().len() + snapshot.indexes().len();
    let mut printed_rows = 0;
    for object in snapshot.objects().iter().take(MAX_PRINTED_ROWS) {
        let mut details = Vec::new();
        if let Some(allowed_length) = object.code_allowed_length {
            details.push(format!("Code={}", allowed_length.as_str()));
        }
        if let Some(allowed_length) = object.number_allowed_length {
            details.push(format!("Number={}", allowed_length.as_str()));
        }
        writeln!(
            output,
            "OBJECT\t{}\t{}\t{}\t{}\t\t{}\t{}\t{}",
            object.guid,
            object.kind.map_or("NonTabular", |kind| kind.as_str()),
            bounded_field(object.name.as_deref().unwrap_or(""), MAX_CELL_WIDTH),
            bounded_field(
                object.physical_table.as_deref().unwrap_or(""),
                MAX_CELL_WIDTH
            ),
            yes_no(object.declared),
            yes_no(object.live),
            bounded_field(&details.join(","), MAX_CELL_WIDTH),
        )?;
        printed_rows += 1;
    }
    for field in snapshot
        .fields()
        .iter()
        .take(MAX_PRINTED_ROWS.saturating_sub(printed_rows))
    {
        writeln!(
            output,
            "FIELD\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t",
            field.guid,
            if field.data_separator {
                "DataSeparator"
            } else {
                "Field"
            },
            bounded_field(field.name.as_deref().unwrap_or(""), MAX_CELL_WIDTH),
            bounded_field(&field.physical_name, MAX_CELL_WIDTH),
            bounded_field(&field.owner_tables.join(","), MAX_CELL_WIDTH),
            yes_no(field.declared),
            yes_no(field.live),
        )?;
        printed_rows += 1;
    }
    for index in snapshot
        .indexes()
        .iter()
        .take(MAX_PRINTED_ROWS.saturating_sub(printed_rows))
    {
        writeln!(
            output,
            "INDEX\t\tIndex\t{}\t{}\t{}\tyes\t{}\t{}",
            bounded_field(&index.declared_name, MAX_CELL_WIDTH),
            bounded_field(index.live_name.as_deref().unwrap_or(""), MAX_CELL_WIDTH),
            bounded_field(&index.table, MAX_CELL_WIDTH),
            yes_no(index.live_name.is_some() && index.unique_matches),
            bounded_field(&index.logical_key.join(","), MAX_CELL_WIDTH),
        )?;
        printed_rows += 1;
    }
    let omitted = total_rows.saturating_sub(printed_rows);
    if omitted != 0 {
        writeln!(output, "# {omitted} rows omitted")?;
    }
    Ok(())
}

pub(crate) const fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

pub(crate) fn escape_field(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '\t' => escaped.push_str("\\t"),
            '\r' => escaped.push_str("\\r"),
            '\n' => escaped.push_str("\\n"),
            value
                if value.is_control()
                    || matches!(
                        value,
                        '\u{2028}' | '\u{2029}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
                    ) =>
            {
                write!(&mut escaped, "\\u{{{:x}}}", u32::from(value))
                    .expect("writing to a String cannot fail");
            }
            value => escaped.push(value),
        }
    }
    escaped
}

pub(crate) fn bounded_field(value: &str, max_width: usize) -> String {
    let mut escaped = escape_field(value);
    if UnicodeWidthStr::width(escaped.as_str()) <= max_width {
        return escaped;
    }
    let ellipsis_width = UnicodeWidthChar::width('…').unwrap_or(1);
    let content_width = max_width.saturating_sub(ellipsis_width);
    let mut width = 0;
    let mut end = 0;
    for (offset, character) in escaped.char_indices() {
        let character_width = UnicodeWidthChar::width(character).unwrap_or(0);
        if width + character_width > content_width {
            break;
        }
        width += character_width;
        end = offset + character.len_utf8();
    }
    escaped.truncate(end);
    if max_width >= ellipsis_width {
        escaped.push('…');
    }
    escaped
}

pub(crate) fn read_lex_source(path: &str) -> Result<String, CliError> {
    if path == "-" {
        let mut source = String::new();
        io::stdin()
            .read_to_string(&mut source)
            .map_err(|error| CliError::Io("cannot read standard input".to_owned(), error))?;
        Ok(source)
    } else {
        fs::read_to_string(path)
            .map_err(|error| CliError::Io(format!("cannot read {path:?}"), error))
    }
}

pub(crate) fn lex(output: &mut impl Write, tokens: &[open_sdbl::Token<'_>]) -> io::Result<()> {
    for token in tokens.iter().take(MAX_PRINTED_ROWS) {
        writeln!(
            output,
            "{}:{}\t{}\t{}",
            token.span.line,
            token.span.column,
            token.kind,
            bounded_field(token.lexeme, MAX_CELL_WIDTH)
        )?;
    }
    let omitted = tokens.len().saturating_sub(MAX_PRINTED_ROWS);
    if omitted != 0 {
        writeln!(output, "# {omitted} rows omitted")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{bounded_field, escape_field};

    #[test]
    fn escapes_controls_and_truncates_at_character_boundaries() {
        assert_eq!(escape_field("\x1b]52;c;x\x07"), "\\u{1b}]52;c;x\\u{7}");
        assert!(bounded_field("界界", 3).ends_with('…'));
    }
}
