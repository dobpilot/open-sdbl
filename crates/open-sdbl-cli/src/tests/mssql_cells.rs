//! Tests of the `mssql cells` module.

use super::*;
use crate::cells::{Cell, DateTimeParts};
use tiberius::ColumnData;
use tiberius::numeric::Numeric;
use tiberius::time::{Date, DateTime, DateTime2, SmallDateTime, Time};

#[test]
fn decodes_tds_values_into_typed_cells() {
    assert_eq!(
        decode_mssql_cell(&ColumnData::Binary(Some(vec![0, 0x7d, 0xd6].into()))),
        Cell::Bytes(vec![0, 0x7d, 0xd6])
    );
    assert_eq!(
        decode_mssql_cell(&ColumnData::Numeric(Some(Numeric::new_with_scale(1550, 2)))),
        Cell::Number("15.50".to_owned())
    );
    assert_eq!(
        decode_mssql_cell(&ColumnData::Numeric(Some(Numeric::new_with_scale(15, 0)))),
        Cell::Number("15".to_owned())
    );
    assert_eq!(
        decode_mssql_cell(&ColumnData::Bit(Some(true))),
        Cell::Bool(true)
    );
    assert_eq!(decode_mssql_cell(&ColumnData::I32(None)), Cell::Null);
    let expected = DateTimeParts {
        year: 2024,
        month: 2,
        day: 29,
        hour: 12,
        minute: 34,
        second: 56,
    };
    // 2024-02-29 is 738_944 days after 0001-01-01 and 45_349 days after 1900-01-01.
    assert_eq!(
        decode_mssql_cell(&ColumnData::DateTime2(Some(DateTime2::new(
            Date::new(738_944),
            Time::new(452_961_234_567, 7),
        )))),
        Cell::DateTime(expected)
    );
    assert_eq!(
        decode_mssql_cell(&ColumnData::DateTime(Some(DateTime::new(
            45_349,
            45_296 * 300 + 150
        )))),
        Cell::DateTime(expected)
    );
    assert_eq!(
        decode_mssql_cell(&ColumnData::SmallDateTime(Some(SmallDateTime::new(
            45_349,
            12 * 60 + 34
        )))),
        Cell::DateTime(DateTimeParts {
            second: 0,
            ..expected
        })
    );
}
