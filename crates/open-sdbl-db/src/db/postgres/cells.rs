//! Decoding PostgreSQL values into the cells a caller prints.

use tokio_postgres::types::{FromSql, Type};

use crate::cells::{Cell, DAYS_FROM_UNIX_EPOCH_TO_2000, DateTimeParts, decode_postgres_numeric};

#[cfg(test)]
#[path = "../../tests/postgres_cells.rs"]
mod tests;

/// Decodes one binary-protocol PostgreSQL value into a typed [`Cell`].
///
/// Only the types the compiler can emit are decoded; anything else is a data
/// error naming the type, so undocumented wire formats never print as garbage.
pub struct PostgresCell(
    /// The decoded value.
    pub Cell,
);

impl<'a> FromSql<'a> for PostgresCell {
    fn from_sql(
        ty: &Type,
        raw: &'a [u8],
    ) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        decode_postgres_cell(ty, raw).map(Self).map_err(Into::into)
    }

    fn from_sql_null(_: &Type) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        Ok(Self(Cell::Null))
    }

    fn accepts(_: &Type) -> bool {
        true
    }
}

pub(super) fn decode_postgres_cell(ty: &Type, raw: &[u8]) -> Result<Cell, String> {
    pub(super) fn array<const N: usize>(raw: &[u8], ty: &Type) -> Result<[u8; N], String> {
        <[u8; N]>::try_from(raw).map_err(|_| {
            format!(
                "PostgreSQL {} value has {} bytes, expected {N}",
                ty.name(),
                raw.len()
            )
        })
    }

    Ok(match *ty {
        Type::BYTEA => Cell::Bytes(raw.to_vec()),
        Type::UUID => Cell::Uuid(array(raw, ty)?),
        Type::BOOL => Cell::Bool(array::<1>(raw, ty)?[0] != 0),
        Type::INT2 => Cell::Number(i16::from_be_bytes(array(raw, ty)?).to_string()),
        Type::INT4 => Cell::Number(i32::from_be_bytes(array(raw, ty)?).to_string()),
        Type::INT8 => Cell::Number(i64::from_be_bytes(array(raw, ty)?).to_string()),
        Type::FLOAT4 => Cell::Number(f32::from_be_bytes(array(raw, ty)?).to_string()),
        Type::FLOAT8 => Cell::Number(f64::from_be_bytes(array(raw, ty)?).to_string()),
        Type::NUMERIC => Cell::Number(decode_postgres_numeric(raw)?),
        Type::TIMESTAMP => {
            let microseconds = i64::from_be_bytes(array(raw, ty)?);
            let seconds = microseconds.div_euclid(1_000_000);
            Cell::DateTime(DateTimeParts::from_unix_days(
                seconds.div_euclid(86_400) + DAYS_FROM_UNIX_EPOCH_TO_2000,
                u32::try_from(seconds.rem_euclid(86_400)).unwrap_or(0),
            ))
        }
        Type::DATE => {
            let days = i64::from(i32::from_be_bytes(array(raw, ty)?));
            Cell::DateTime(DateTimeParts::from_unix_days(
                days + DAYS_FROM_UNIX_EPOCH_TO_2000,
                0,
            ))
        }
        Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME | Type::UNKNOWN => Cell::Text(
            std::str::from_utf8(raw)
                .map_err(|error| format!("PostgreSQL {} value is not UTF-8: {error}", ty.name()))?
                .to_owned(),
        ),
        _ => {
            return Err(format!(
                "unsupported PostgreSQL column type {}; the compiler should have cast it",
                ty.name()
            ));
        }
    })
}
