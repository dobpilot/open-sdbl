//! Decoding TDS values into the cells a caller prints.

use tiberius::ColumnData;

use crate::cells::{
    Cell, DAYS_FROM_1900_TO_UNIX_EPOCH, DAYS_FROM_YEAR_ONE_TO_UNIX_EPOCH, DateTimeParts,
    format_scaled_integer,
};
use crate::error::DbError;

#[cfg(test)]
#[path = "../../tests/mssql_cells.rs"]
mod tests;

pub(super) fn should_disconnect_after_mssql_error(error: &DbError) -> bool {
    error.requires_mssql_disconnect()
}

/// Decodes the first `column_count` values of one TDS row.
pub fn mssql_row(row: &tiberius::Row, column_count: usize) -> Result<Vec<Cell>, DbError> {
    if row.columns().len() < column_count {
        return Err(DbError::Data(format!(
            "MSSQL returned {} columns, but {column_count} were expected",
            row.columns().len()
        )));
    }
    row.cells()
        .take(column_count)
        .map(|(_, data)| Ok(decode_mssql_cell(data)))
        .collect()
}

/// Decodes one TDS value into a typed [`Cell`].
pub fn decode_mssql_cell(data: &ColumnData<'_>) -> Cell {
    pub(super) fn nullable<T>(value: Option<T>, convert: impl FnOnce(T) -> Cell) -> Cell {
        value.map_or(Cell::Null, convert)
    }

    match data {
        ColumnData::U8(value) => nullable(*value, |value| Cell::Number(value.to_string())),
        ColumnData::I16(value) => nullable(*value, |value| Cell::Number(value.to_string())),
        ColumnData::I32(value) => nullable(*value, |value| Cell::Number(value.to_string())),
        ColumnData::I64(value) => nullable(*value, |value| Cell::Number(value.to_string())),
        ColumnData::F32(value) => nullable(*value, |value| Cell::Number(value.to_string())),
        ColumnData::F64(value) => nullable(*value, |value| Cell::Number(value.to_string())),
        ColumnData::Bit(value) => nullable(*value, Cell::Bool),
        ColumnData::String(value) => {
            nullable(value.as_deref(), |value| Cell::Text(value.to_owned()))
        }
        ColumnData::Guid(value) => nullable(*value, |guid| Cell::Uuid(*guid.as_bytes())),
        ColumnData::Binary(value) => {
            nullable(value.as_deref(), |value| Cell::Bytes(value.to_vec()))
        }
        ColumnData::Numeric(value) => nullable(*value, |value| {
            Cell::Number(format_scaled_integer(
                value.value(),
                u32::from(value.scale()),
            ))
        }),
        ColumnData::Xml(value) => nullable(value.as_deref(), |value| Cell::Text(value.to_string())),
        ColumnData::DateTime(value) => nullable(*value, |value| {
            Cell::DateTime(DateTimeParts::from_unix_days(
                i64::from(value.days()) - DAYS_FROM_1900_TO_UNIX_EPOCH,
                value.seconds_fragments() / 300,
            ))
        }),
        ColumnData::SmallDateTime(value) => nullable(*value, |value| {
            Cell::DateTime(DateTimeParts::from_unix_days(
                i64::from(value.days()) - DAYS_FROM_1900_TO_UNIX_EPOCH,
                u32::from(value.seconds_fragments()) * 60,
            ))
        }),
        ColumnData::Date(value) => nullable(*value, |value| {
            Cell::DateTime(DateTimeParts::from_unix_days(
                i64::from(value.days()) - DAYS_FROM_YEAR_ONE_TO_UNIX_EPOCH,
                0,
            ))
        }),
        ColumnData::Time(value) => nullable(*value, |value| {
            Cell::DateTime(DateTimeParts::from_unix_days(0, mssql_time_seconds(value)))
        }),
        ColumnData::DateTime2(value) => nullable(*value, mssql_datetime2),
        ColumnData::DateTimeOffset(value) => {
            nullable(*value, |value| mssql_datetime2(value.datetime2()))
        }
    }
}

pub(super) fn mssql_time_seconds(time: tiberius::time::Time) -> u32 {
    let divisor = 10_u64.pow(u32::from(time.scale()));
    u32::try_from(time.increments() / divisor.max(1)).unwrap_or(0)
}

pub(super) fn mssql_datetime2(value: tiberius::time::DateTime2) -> Cell {
    Cell::DateTime(DateTimeParts::from_unix_days(
        i64::from(value.date().days()) - DAYS_FROM_YEAR_ONE_TO_UNIX_EPOCH,
        mssql_time_seconds(value.time()),
    ))
}
