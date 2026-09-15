//! Tests of the `cells` module.

use super::{
    Cell, DateTimeParts, civil_from_days, decode_postgres_numeric, format_binary,
    format_scaled_integer, format_uuid,
};

fn numeric(digits: &[u16], weight: i16, sign: u16, scale: u16) -> Vec<u8> {
    let mut raw = Vec::new();
    raw.extend_from_slice(&(digits.len() as u16).to_be_bytes());
    raw.extend_from_slice(&weight.to_be_bytes());
    raw.extend_from_slice(&sign.to_be_bytes());
    raw.extend_from_slice(&scale.to_be_bytes());
    for digit in digits {
        raw.extend_from_slice(&digit.to_be_bytes());
    }
    raw
}

#[test]
fn renders_cells_with_one_policy() {
    assert_eq!(Cell::Null.render(), "NULL");
    assert_eq!(Cell::Bytes(vec![0, 0x7d, 0xd6]).render(), "0x007DD6");
    assert_eq!(Cell::Bool(true).render(), "true");
    assert_eq!(Cell::Bool(false).render(), "false");
    assert_eq!(Cell::Number("15.50".to_owned()).render(), "15.50");
    assert_eq!(
        Cell::DateTime(DateTimeParts::from_unix_days(19_782, 45_296)).render(),
        "2024-02-29 12:34:56"
    );
    let uuid = [
        0xd2, 0xf8, 0xbd, 0xe9, 0xfa, 0xdd, 0x4b, 0xe8, 0x90, 0x22, 0x24, 0x9e, 0x3a, 0x1a, 0xc4,
        0xb9,
    ];
    assert_eq!(
        Cell::Uuid(uuid).render(),
        "d2f8bde9-fadd-4be8-9022-249e3a1ac4b9"
    );
    assert_eq!(
        format_uuid(&[0; 16]),
        "00000000-0000-0000-0000-000000000000"
    );
    assert_eq!(format_binary(&[]), "0x");
}

#[test]
fn converts_days_to_civil_dates() {
    assert_eq!(civil_from_days(0), (1970, 1, 1));
    assert_eq!(civil_from_days(-1), (1969, 12, 31));
    assert_eq!(civil_from_days(10_957), (2000, 1, 1));
    assert_eq!(civil_from_days(19_782), (2024, 2, 29));
    assert_eq!(civil_from_days(-719_162), (1, 1, 1));
    assert_eq!(civil_from_days(-25_567), (1900, 1, 1));
    assert_eq!(civil_from_days(2_932_896), (9999, 12, 31));
}

#[test]
fn formats_scaled_integers_with_declared_scale() {
    assert_eq!(format_scaled_integer(1550, 2), "15.50");
    assert_eq!(format_scaled_integer(-5, 1), "-0.5");
    assert_eq!(format_scaled_integer(15, 0), "15");
    assert_eq!(format_scaled_integer(0, 3), "0.000");
}

#[test]
fn decodes_postgres_numeric_wire_format() {
    assert_eq!(
        decode_postgres_numeric(&numeric(&[15, 5000], 0, 0, 2)).unwrap(),
        "15.50"
    );
    assert_eq!(
        decode_postgres_numeric(&numeric(&[12, 3456], 1, 0x4000, 0)).unwrap(),
        "-123456"
    );
    assert_eq!(
        decode_postgres_numeric(&numeric(&[5], -1, 0, 4)).unwrap(),
        "0.0005"
    );
    assert_eq!(
        decode_postgres_numeric(&numeric(&[1], 0, 0, 3)).unwrap(),
        "1.000"
    );
    assert_eq!(
        decode_postgres_numeric(&numeric(&[], 0, 0, 0)).unwrap(),
        "0"
    );
    assert_eq!(
        decode_postgres_numeric(&numeric(&[], 0, 0xC000, 0)).unwrap(),
        "NaN"
    );
    assert!(decode_postgres_numeric(&[0, 1]).is_err());
}
