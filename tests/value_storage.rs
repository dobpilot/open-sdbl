//! Values a `ХранилищеЗначения` column stores: the reference it holds,
//! the statements reading the content, and the decoded value.

use open_sdbl::metadata::{
    DEFAULT_OUTPUT_LIMIT, MsSqlMetadataQueries, PostgresMetadataQueries, StoredValueRef,
    decode_stored_value, entry_string, parse_stored_value_ref, stored_map_entries,
};

/// The 64 bytes a column of the УНФ demo holds, as the base answers them.
fn reference_bytes() -> Vec<u8> {
    let hex = "53544f524844520000000000000000009b192a4234e0384986c5caee6a45b9dda458010000000000ffffffffffffffffffffffffffffffff0000000000000000";
    (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap())
        .collect()
}

/// The content of a stored value: the header, the mark and the text.
fn content(text: &str) -> Vec<u8> {
    let mut bytes = vec![0x01, 0x01];
    bytes.extend_from_slice(&(text.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&[0xef, 0xbb, 0xbf]);
    bytes.extend_from_slice(text.as_bytes());
    bytes
}

const MAP: &str = r##"{"#",3ee983d7-ace7-40f9-bb7e-2e916fcddd56,
{2,
{
{"S","ВерсииШаблонов"},
{"S",",ДляОбъекта9,"}
},
{
{"S","Пусто"},
{"S",""}
}
}
}"##;

#[test]
fn reads_the_reference_a_column_holds() {
    let reference = parse_stored_value_ref(&reference_bytes()).unwrap();
    assert_eq!(reference.key_hex(), "9b192a4234e0384986c5caee6a45b9dd");
    assert_eq!(reference.length, 88_228);
    // Anything else is no reference: SQL Server keeps the value of the
    // column in the column itself, and it is decoded as it stands.
    let inline = content(MAP);
    assert!(parse_stored_value_ref(&inline).is_none());
    assert!(
        !stored_map_entries(&decode_stored_value(&inline, DEFAULT_OUTPUT_LIMIT).unwrap())
            .is_empty()
    );
    assert!(parse_stored_value_ref(&[]).is_none());
    assert!(parse_stored_value_ref(b"STORHDR").is_none());
    assert!(parse_stored_value_ref(&[0; 64]).is_none());
}

#[test]
fn builds_the_statements_reading_the_content() {
    let reference = parse_stored_value_ref(&reference_bytes()).unwrap();
    assert_eq!(
        PostgresMetadataQueries::stored_value(&reference),
        "SELECT f_data FROM binarydata WHERE f_key = decode('9b192a4234e0384986c5caee6a45b9dd', 'hex') ORDER BY f_off"
    );
    assert_eq!(
        MsSqlMetadataQueries::stored_value(&reference),
        "SELECT [f_data] FROM [dbo].[BinaryData] WHERE [f_key] = 0x9B192A4234E0384986C5CAEE6A45B9DD ORDER BY [f_off]"
    );
}

#[test]
fn decodes_a_stored_map() {
    let value = decode_stored_value(&content(MAP), DEFAULT_OUTPUT_LIMIT).unwrap();
    let entries = stored_map_entries(&value);
    let named = entries
        .iter()
        .map(|(key, value)| (key.as_str(), entry_string(value).unwrap_or_default()))
        .collect::<Vec<_>>();
    assert_eq!(
        named,
        [
            ("ВерсииШаблонов", ",ДляОбъекта9,".to_owned()),
            ("Пусто", String::new())
        ]
    );
    // A value that is not a map answers no entries.
    let empty = decode_stored_value(&content("{0}"), DEFAULT_OUTPUT_LIMIT).unwrap();
    assert!(stored_map_entries(&empty).is_empty());
}

#[test]
fn refuses_content_that_is_neither_the_serialization_nor_deflate() {
    let error = decode_stored_value(&[0; 4], DEFAULT_OUTPUT_LIMIT).unwrap_err();
    assert!(
        error.message().contains("shorter than its header"),
        "{error}"
    );
    let mut broken = content("{1}");
    broken[10] = b'x';
    broken[11] = b'y';
    assert!(decode_stored_value(&broken, DEFAULT_OUTPUT_LIMIT).is_err());
    // The reference of a stored value is not the content.
    assert!(
        StoredValueRef::key_hex(&parse_stored_value_ref(&reference_bytes()).unwrap()).len() == 32
    );
}
