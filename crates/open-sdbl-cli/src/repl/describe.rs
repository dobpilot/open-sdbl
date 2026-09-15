//! Printing what the metadata says: tables, indexes, descriptions and
//! the constants table.

use std::io::{self, Write};

use open_sdbl::metadata::{MetadataObject, MetadataSnapshot};
use open_sdbl::query::{
    ColumnKind, TempTablesManager, constants_table_fields, find_metadata_object, queryable_fields,
};

use crate::error::CliError;
use crate::output::{MAX_CELL_WIDTH, bounded_field, escape_field, yes_no};

use super::*;

#[cfg(test)]
#[path = "../tests/repl_describe.rs"]
mod tests;

pub(super) fn print_tables(output: &mut impl Write, snapshot: &MetadataSnapshot) -> io::Result<()> {
    let mut rows: Vec<Vec<String>> = snapshot
        .objects()
        .iter()
        .filter_map(|object| {
            Some(vec![
                object.kind?.as_str().to_owned(),
                object.name.clone().unwrap_or_default(),
                object.guid.to_string(),
                object.physical_table.clone()?,
                yes_no(object.declared).to_owned(),
                yes_no(object.live).to_owned(),
            ])
        })
        .collect();
    rows.sort_by(|left, right| (&left[0], &left[1]).cmp(&(&right[0], &right[1])));
    print_table(
        output,
        &["Kind", "Name", "GUID", "Table", "Schema", "Live"],
        &rows,
    )?;
    writeln!(output, "({} objects)", rows.len())
}

/// The names of the temporary tables a statement can read.
pub(super) fn temporary_table_names(temporary: &TempTablesManager) -> Vec<String> {
    temporary
        .tables()
        .map(|table| table.name().to_owned())
        .collect()
}

/// Prints the temporary tables of this session with their columns.
pub(super) fn print_temporary_tables(
    output: &mut impl Write,
    temporary: &TempTablesManager,
) -> io::Result<()> {
    if temporary.is_empty() {
        return writeln!(output, "No temporary tables placed.");
    }
    let width = temporary
        .tables()
        .map(|table| table.name().chars().count())
        .max()
        .unwrap_or(0);
    for table in temporary.tables() {
        let columns = table
            .columns()
            .iter()
            .map(|column| {
                format!(
                    "{} [{}]",
                    escape_field(&column.label),
                    column_kind_label(&column.kind)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            output,
            "{:<width$}  {columns}",
            escape_field(table.name()),
            width = width
        )?;
    }
    Ok(())
}

/// A short display name for a column kind, used by `\tables`.
pub(super) fn column_kind_label(kind: &ColumnKind) -> String {
    match kind {
        ColumnKind::String { .. } => "String".to_owned(),
        ColumnKind::Number { .. } => "Number".to_owned(),
        ColumnKind::Boolean => "Boolean".to_owned(),
        ColumnKind::DateTime => "DateTime".to_owned(),
        ColumnKind::Reference { runtime_typed, .. } => {
            if *runtime_typed {
                "Reference*".to_owned()
            } else {
                "Reference".to_owned()
            }
        }
        ColumnKind::Binary { .. } => "Binary".to_owned(),
        ColumnKind::Uuid => "UUID".to_owned(),
        ColumnKind::Null => "Null".to_owned(),
        ColumnKind::Undefined => "Undefined".to_owned(),
        ColumnKind::Type => "Type".to_owned(),
        _ => "Unknown".to_owned(),
    }
}

pub(super) fn print_indexes(
    output: &mut impl Write,
    snapshot: &MetadataSnapshot,
) -> io::Result<()> {
    let mut rows: Vec<Vec<String>> = snapshot
        .indexes()
        .iter()
        .map(|index| {
            vec![
                object_display_name(object_for_table(snapshot, &index.table)),
                index.table.clone(),
                index.declared_name.clone(),
                index.live_name.clone().unwrap_or_default(),
                index.logical_key.join(", "),
                yes_no(index.live_name.is_some() && index.unique_matches).to_owned(),
            ]
        })
        .collect();
    rows.sort_by(|left, right| (&left[0], &left[2]).cmp(&(&right[0], &right[2])));
    print_table(
        output,
        &["Metadata", "Table", "Declared", "Live", "Key", "Match"],
        &rows,
    )?;
    writeln!(output, "({} indexes)", rows.len())
}

/// Spellings of the constants table accepted by `\d` and offered by
/// completion.
pub(super) const CONSTANTS_TABLE_NAMES: [&str; 2] = ["Константы", "Constants"];

pub(super) fn is_constants_table_name(name: &str) -> bool {
    let name = name.to_lowercase();
    CONSTANTS_TABLE_NAMES
        .iter()
        .any(|candidate| candidate.to_lowercase() == name)
}

/// `\d Константы`: every live constant with its value field.
pub(super) fn print_constants_description(
    output: &mut impl Write,
    snapshot: &MetadataSnapshot,
) -> Result<(), CliError> {
    let constants =
        constants_table_fields(snapshot).map_err(|error| CliError::Data(error.to_string()))?;
    writeln!(
        output,
        "Константы  constants={}  one row: UNION ALL of the referenced _Const tables aggregated with MAX",
        constants.len()
    )
    .map_err(CliError::standard_output)?;
    let rows: Vec<Vec<String>> = constants
        .into_iter()
        .map(|(object, field)| {
            vec![
                field.name,
                object.physical_table.clone().unwrap_or_default(),
                field
                    .columns
                    .into_iter()
                    .map(|column| format!("{}:{}", column.physical_name, column.data_type))
                    .collect::<Vec<_>>()
                    .join(", "),
                field.reference_target.unwrap_or_default(),
            ]
        })
        .collect();
    print_table(
        output,
        &["Name", "Table", "Physical members", "Reference target"],
        &rows,
    )
    .map_err(CliError::standard_output)
}

pub(super) fn print_description(
    output: &mut impl Write,
    snapshot: &MetadataSnapshot,
    name: &str,
) -> Result<(), CliError> {
    if is_constants_table_name(name) {
        return print_constants_description(output, snapshot);
    }
    let object =
        find_metadata_object(snapshot, name).map_err(|error| CliError::Data(error.to_string()))?;
    let fields =
        queryable_fields(snapshot, object).map_err(|error| CliError::Data(error.to_string()))?;
    writeln!(
        output,
        "{}  GUID={}  table={}  schema={}  live={}",
        bounded_field(&object_display_name(Some(object)), MAX_CELL_WIDTH),
        object.guid,
        bounded_field(
            object.physical_table.as_deref().unwrap_or(""),
            MAX_CELL_WIDTH
        ),
        yes_no(object.declared),
        yes_no(object.live),
    )
    .map_err(CliError::standard_output)?;

    let field_rows: Vec<Vec<String>> = fields
        .into_iter()
        .map(|field| {
            let origin = field
                .schema_name
                .strip_prefix("Fld")
                .and_then(|number| number.parse::<u32>().ok())
                .and_then(|number| {
                    snapshot
                        .fields()
                        .iter()
                        .find(|metadata| metadata.number == number)
                })
                .and_then(|metadata| metadata.extension_origin.clone())
                .unwrap_or_default();
            vec![
                field.name,
                field.schema_name,
                field.aliases.join(", "),
                field
                    .columns
                    .into_iter()
                    .map(|column| format!("{}:{}", column.physical_name, column.data_type))
                    .collect::<Vec<_>>()
                    .join(", "),
                field.reference_target.unwrap_or_default(),
                origin,
            ]
        })
        .collect();
    writeln!(output, "Attributes:").map_err(CliError::standard_output)?;
    print_table(
        output,
        &[
            "Name",
            "Schema name",
            "Aliases",
            "Physical members",
            "Reference target",
            "Extension",
        ],
        &field_rows,
    )
    .map_err(CliError::standard_output)?;

    let table = object.physical_table.as_deref().unwrap_or("");
    let index_rows: Vec<Vec<String>> = snapshot
        .indexes()
        .iter()
        .filter(|index| index.table.eq_ignore_ascii_case(table))
        .map(|index| {
            vec![
                index.declared_name.clone(),
                index.live_name.clone().unwrap_or_default(),
                index.logical_key.join(", "),
                yes_no(index.live_name.is_some() && index.unique_matches).to_owned(),
            ]
        })
        .collect();
    writeln!(output, "Indexes:").map_err(CliError::standard_output)?;
    print_table(output, &["Declared", "Live", "Key", "Match"], &index_rows)
        .map_err(CliError::standard_output)?;
    Ok(())
}

pub(super) fn object_for_table<'snapshot>(
    snapshot: &'snapshot MetadataSnapshot,
    table: &str,
) -> Option<&'snapshot MetadataObject> {
    snapshot.objects().iter().find(|object| {
        object
            .physical_table
            .as_deref()
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(table))
    })
}

pub(super) fn object_display_name(object: Option<&MetadataObject>) -> String {
    let Some(object) = object else {
        return String::new();
    };
    match (object.kind, object.name.as_deref()) {
        (Some(kind), Some(name)) => format!("{}.{name}", kind.as_str()),
        (_, Some(name)) => name.to_owned(),
        _ => String::new(),
    }
}
