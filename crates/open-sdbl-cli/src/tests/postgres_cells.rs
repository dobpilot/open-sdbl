//! Tests of the `postgres cells` module.

use super::*;
use crate::cells::{Cell, DateTimeParts};
use tokio_postgres::types::Type;

#[test]
fn decodes_binary_protocol_values_into_typed_cells() {
    assert_eq!(
        decode_postgres_cell(&Type::BYTEA, &[0, 0x7d, 0xd6]).unwrap(),
        Cell::Bytes(vec![0, 0x7d, 0xd6])
    );
    assert_eq!(
        decode_postgres_cell(&Type::UUID, &[9; 16]).unwrap(),
        Cell::Uuid([9; 16])
    );
    assert_eq!(
        decode_postgres_cell(&Type::BOOL, &[1]).unwrap(),
        Cell::Bool(true)
    );
    assert_eq!(
        decode_postgres_cell(&Type::INT8, &(-42_i64).to_be_bytes()).unwrap(),
        Cell::Number("-42".to_owned())
    );
    assert_eq!(
        decode_postgres_cell(&Type::FLOAT8, &1.5_f64.to_be_bytes()).unwrap(),
        Cell::Number("1.5".to_owned())
    );
    // 2024-02-29 12:34:56 is 762_525_296 seconds after 2000-01-01.
    assert_eq!(
        decode_postgres_cell(&Type::TIMESTAMP, &(762_525_296_000_000_i64).to_be_bytes()).unwrap(),
        Cell::DateTime(DateTimeParts {
            year: 2024,
            month: 2,
            day: 29,
            hour: 12,
            minute: 34,
            second: 56,
        })
    );
    assert_eq!(
        decode_postgres_cell(&Type::DATE, &(-1_i32).to_be_bytes()).unwrap(),
        Cell::DateTime(DateTimeParts {
            year: 1999,
            month: 12,
            day: 31,
            hour: 0,
            minute: 0,
            second: 0,
        })
    );
    assert_eq!(
        decode_postgres_cell(&Type::TEXT, "Код".as_bytes()).unwrap(),
        Cell::Text("Код".to_owned())
    );
    assert!(
        decode_postgres_cell(&Type::JSON, b"{}")
            .unwrap_err()
            .contains("json")
    );
    assert!(decode_postgres_cell(&Type::INT4, &[1, 2]).is_err());
}
