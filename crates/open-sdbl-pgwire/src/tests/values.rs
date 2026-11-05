use super::*;

fn number(unscaled: i128, scale: u8) -> ParameterValue {
    ParameterValue::Number { unscaled, scale }
}

#[test]
fn columns_are_described_by_kind() {
    let cases = [
        (
            ColumnKind::Reference {
                targets: Vec::new(),
                runtime_typed: false,
            },
            oid::BYTEA,
            -1,
        ),
        (ColumnKind::Binary { length: Some(16) }, oid::BYTEA, -1),
        (ColumnKind::String { length: None }, oid::TEXT, -1),
        (ColumnKind::String { length: Some(0) }, oid::TEXT, -1),
        (ColumnKind::String { length: Some(150) }, oid::VARCHAR, 154),
        (
            ColumnKind::Number {
                precision: Some(15),
                scale: Some(2),
            },
            oid::NUMERIC,
            (15 << 16 | 2) + 4,
        ),
        (
            ColumnKind::Number {
                precision: None,
                scale: Some(0),
            },
            oid::NUMERIC,
            -1,
        ),
        (ColumnKind::Boolean, oid::BOOL, -1),
        (ColumnKind::DateTime, oid::TIMESTAMP, -1),
        (ColumnKind::Uuid, oid::UUID, -1),
        (ColumnKind::Type, oid::TEXT, -1),
        (ColumnKind::Undefined, oid::TEXT, -1),
        (ColumnKind::Null, oid::TEXT, -1),
    ];
    for (kind, expected_oid, modifier) in cases {
        let (field, _) = describe_column("c", &kind, 1);
        assert_eq!(field.type_oid, expected_oid, "{kind:?}");
        assert_eq!(field.type_modifier, modifier, "{kind:?}");
        assert_eq!(field.format, 1);
    }
}

fn reference() -> ColumnKind {
    ColumnKind::Reference {
        targets: Vec::new(),
        runtime_typed: false,
    }
}

#[test]
fn cells_encode_in_text_and_binary() {
    let wire = |kind: &ColumnKind| describe_column("c", kind, 0).1;
    let bytes = Cell::Bytes((0..16).collect());
    assert_eq!(
        encode_cell(&bytes, wire(&reference()), false).unwrap(),
        b"\\x000102030405060708090a0b0c0d0e0f".to_vec()
    );
    assert_eq!(
        encode_cell(&bytes, wire(&reference()), true).unwrap(),
        (0..16).collect::<Vec<u8>>()
    );
    let boolean = wire(&ColumnKind::Boolean);
    assert_eq!(
        encode_cell(&Cell::Bool(true), boolean, false).unwrap(),
        b"t"
    );
    assert_eq!(encode_cell(&Cell::Bool(false), boolean, true).unwrap(), [0]);
    assert_eq!(
        encode_cell(&Cell::Number("1".to_owned()), boolean, false).unwrap(),
        b"t"
    );
    let date = Cell::DateTime(DateTimeParts {
        year: 2000,
        month: 1,
        day: 2,
        hour: 0,
        minute: 0,
        second: 1,
    });
    let timestamp = wire(&ColumnKind::DateTime);
    assert_eq!(
        encode_cell(&date, timestamp, false).unwrap(),
        b"2000-01-02 00:00:01"
    );
    assert_eq!(
        encode_cell(&date, timestamp, true).unwrap(),
        86_401_000_000_i64.to_be_bytes()
    );
    let early = Cell::DateTime(DateTimeParts {
        year: 1,
        month: 1,
        day: 1,
        hour: 0,
        minute: 0,
        second: 0,
    });
    // 0001-01-01 is 730119 days before 2000-01-01.
    assert_eq!(
        encode_cell(&early, timestamp, true).unwrap(),
        (-730_119_i64 * 86_400 * 1_000_000).to_be_bytes()
    );
    let uuid = Cell::Uuid([0xab; 16]);
    assert_eq!(
        encode_cell(&uuid, wire(&ColumnKind::Uuid), false).unwrap(),
        b"abababab-abab-abab-abab-abababababab"
    );
    assert_eq!(
        encode_cell(&uuid, wire(&ColumnKind::Uuid), true).unwrap(),
        [0xab; 16]
    );
    assert_eq!(
        encode_cell(
            &Cell::Text("Номенклатура".to_owned()),
            wire(&ColumnKind::String { length: None }),
            true
        )
        .unwrap(),
        "Номенклатура".as_bytes()
    );
    assert_eq!(encode_cell(&Cell::Null, timestamp, true), None);
    // A type value is sent as the console renders it.
    assert_eq!(
        encode_cell(&Cell::Bytes(vec![1, 2]), wire(&ColumnKind::Type), false).unwrap(),
        b"0x0102"
    );
}

#[test]
fn numeric_binary_matches_postgresql() {
    // Expected bytes as PostgreSQL's numeric_send writes them.
    let cases: [(&str, &[u16]); 7] = [
        ("0", &[0, 0, 0, 0]),
        ("0.00", &[0, 0, 0, 2]),
        ("1", &[1, 0, 0, 0, 1]),
        ("-12345.67", &[3, 1, 0x4000, 2, 1, 2345, 6700]),
        ("10000", &[1, 1, 0, 0, 1]),
        ("0.0001", &[1, 0xffff, 0, 4, 1]),
        ("15.50", &[2, 0, 0, 2, 15, 5000]),
    ];
    for (text, words) in cases {
        let expected = words
            .iter()
            .flat_map(|word| word.to_be_bytes())
            .collect::<Vec<_>>();
        assert_eq!(encode_numeric(text).unwrap(), expected, "{text}");
        assert_eq!(&decode_numeric(&expected).unwrap(), text, "{text}");
    }
    let large = encode_numeric("123456789012.50").unwrap();
    assert_eq!(decode_numeric(&large).unwrap(), "123456789012.50");
    assert_eq!(encode_numeric("-0.000"), encode_numeric("0.000"));
    assert!(encode_numeric("1e5").is_none());
    assert!(encode_numeric("").is_none());
}

#[test]
fn scalars_bind_by_declared_type() {
    let text = |oid: u32, value: &str| decode_parameter(oid, false, Some(value.as_bytes()));
    assert_eq!(text(oid::BOOL, "t"), Ok(ParameterValue::Boolean(true)));
    assert_eq!(text(oid::BOOL, "off"), Ok(ParameterValue::Boolean(false)));
    assert_eq!(text(oid::INT4, "-42"), Ok(number(-42, 0)));
    assert_eq!(
        text(oid::INT8, "9223372036854775807"),
        Ok(number(i128::from(i64::MAX), 0))
    );
    assert_eq!(text(oid::NUMERIC, "15.50"), Ok(number(155, 1)));
    assert_eq!(text(oid::NUMERIC, "-0.05"), Ok(number(-5, 2)));
    assert_eq!(text(oid::FLOAT8, "0.5"), Ok(number(5, 1)));
    assert_eq!(
        text(oid::TEXT, "Склад"),
        Ok(ParameterValue::String("Склад".to_owned()))
    );
    assert_eq!(
        text(oid::UNSPECIFIED, "x"),
        Ok(ParameterValue::String("x".to_owned()))
    );
    assert_eq!(
        text(oid::VARCHAR, ""),
        Ok(ParameterValue::String(String::new()))
    );
    assert_eq!(
        text(oid::BYTEA, "\\x00ff"),
        Ok(ParameterValue::Binary(vec![0, 0xff]))
    );
    assert_eq!(
        text(oid::BYTEA, "a\\\\b\\001"),
        Ok(ParameterValue::Binary(b"a\\b\x01".to_vec()))
    );
    let date = |y, mo, d, h, mi, s| {
        Ok(ParameterValue::Date(
            open_sdbl::query::ParameterDate::new(y, mo, d, h, mi, s).unwrap(),
        ))
    };
    assert_eq!(text(oid::DATE, "2024-02-29"), date(2024, 2, 29, 0, 0, 0));
    assert_eq!(
        text(oid::TIMESTAMP, "2024-02-29 13:14:15.999"),
        date(2024, 2, 29, 13, 14, 15)
    );
    assert_eq!(
        text(oid::TIMESTAMPTZ, "2024-03-01 01:30:00+03"),
        date(2024, 2, 29, 22, 30, 0)
    );
    assert_eq!(
        text(oid::TIMESTAMPTZ, "2024-03-01 01:30:00-05:30"),
        date(2024, 3, 1, 7, 0, 0)
    );
    assert_eq!(
        text(oid::UUID, "00112233-4455-6677-8899-aabbccddeeff"),
        Ok(ParameterValue::Uuid(
            "00112233-4455-6677-8899-aabbccddeeff".parse().unwrap()
        ))
    );
    assert_eq!(
        decode_parameter(oid::INT4, false, None),
        Ok(ParameterValue::Null)
    );
}

#[test]
fn binary_scalars_bind() {
    let binary = |oid: u32, bytes: &[u8]| decode_parameter(oid, true, Some(bytes));
    assert_eq!(binary(oid::BOOL, &[1]), Ok(ParameterValue::Boolean(true)));
    assert_eq!(binary(oid::INT2, &7_i16.to_be_bytes()), Ok(number(7, 0)));
    assert_eq!(
        binary(oid::INT4, &(-7_i32).to_be_bytes()),
        Ok(number(-7, 0))
    );
    assert_eq!(binary(oid::INT8, &7_i64.to_be_bytes()), Ok(number(7, 0)));
    assert_eq!(
        binary(oid::FLOAT8, &2.25_f64.to_be_bytes()),
        Ok(number(225, 2))
    );
    let numeric = encode_numeric("-12345.67").unwrap();
    assert_eq!(binary(oid::NUMERIC, &numeric), Ok(number(-1_234_567, 2)));
    assert_eq!(
        binary(oid::TIMESTAMP, &86_401_000_000_i64.to_be_bytes()),
        Ok(ParameterValue::Date(
            open_sdbl::query::ParameterDate::new(2000, 1, 2, 0, 0, 1).unwrap()
        ))
    );
    assert_eq!(
        binary(oid::DATE, &(-1_i32).to_be_bytes()),
        Ok(ParameterValue::Date(
            open_sdbl::query::ParameterDate::new(1999, 12, 31, 0, 0, 0).unwrap()
        ))
    );
    // A 16-byte bytea stays raw: the compiler reads it as a bare RRRef.
    assert_eq!(
        binary(oid::BYTEA, &[7; 16]),
        Ok(ParameterValue::Binary(vec![7; 16]))
    );
    assert_eq!(binary(oid::UUID, &[0; 16]).map(|_| ()), Ok(()));
    // An unspecified type is the `text` Describe announced.
    assert_eq!(
        binary(oid::UNSPECIFIED, b"x"),
        Ok(ParameterValue::String("x".to_owned()))
    );
}

#[test]
fn arrays_bind_as_lists() {
    assert_eq!(
        decode_parameter(1009, false, Some(b"{a,\"b c\",NULL,\"NULL\",\"q\\\"\"}")),
        Ok(ParameterValue::List(vec![
            ParameterValue::String("a".to_owned()),
            ParameterValue::String("b c".to_owned()),
            ParameterValue::Null,
            ParameterValue::String("NULL".to_owned()),
            ParameterValue::String("q\"".to_owned()),
        ]))
    );
    assert_eq!(
        decode_parameter(1007, false, Some(b"{}")),
        Ok(ParameterValue::List(Vec::new()))
    );
    // A binary int4[] of two elements, the second NULL.
    let mut binary = Vec::new();
    for word in [1_i32, 1, 23, 2, 1, 4, 5, -1] {
        binary.extend_from_slice(&word.to_be_bytes());
    }
    assert_eq!(
        decode_parameter(1007, true, Some(&binary)),
        Ok(ParameterValue::List(vec![
            number(5, 0),
            ParameterValue::Null
        ]))
    );
    assert!(decode_parameter(1007, false, Some(b"{{1},{2}}")).is_err());
}

#[test]
fn unsupported_and_invalid_values_are_distinguished() {
    const JSONB: u32 = 3802;
    assert_eq!(
        decode_parameter(JSONB, false, Some(b"{}")),
        Err(BindError::UnsupportedType(JSONB))
    );
    assert_eq!(
        decode_parameter(JSONB, false, None),
        Err(BindError::UnsupportedType(JSONB))
    );
    for (oid, value) in [
        (oid::INT4, "x"),
        (oid::BOOL, "maybe"),
        (oid::NUMERIC, "1e3"),
        (oid::FLOAT8, "inf"),
        (oid::FLOAT8, "1e300"),
        (oid::DATE, "2024-13-01"),
        (oid::TIMESTAMP, "0000-01-01 00:00:00"),
        (oid::UUID, "nope"),
        (oid::BYTEA, "\\xz"),
    ] {
        assert!(
            matches!(
                decode_parameter(oid, false, Some(value.as_bytes())),
                Err(BindError::Invalid(_))
            ),
            "{oid} {value}"
        );
    }
}

#[test]
fn formats_apply_to_one_or_each_column() {
    assert_eq!(format_of(&[], 3), 0);
    assert_eq!(format_of(&[1], 3), 1);
    assert_eq!(format_of(&[0, 1], 1), 1);
}
