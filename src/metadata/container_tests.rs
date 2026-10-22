use super::*;

const SCALAR: &[u8] =
    include_bytes!("../../tests/fixtures/common-settings-native-1015/scalar.settingsdata");
const STORAGE: &[u8] =
    include_bytes!("../../tests/fixtures/common-settings-native-1015/storage.settingsdata");

#[test]
fn native_scalar_and_storage_entries_match_the_measured_bytes() {
    for (container, name, expected) in [
        (
            SCALAR,
            "$Stream$",
            include_bytes!(
                "../../tests/fixtures/common-settings-native-1015/scalar.$Stream$.stream"
            )
            .as_slice(),
        ),
        (
            STORAGE,
            "$Stream$",
            include_bytes!(
                "../../tests/fixtures/common-settings-native-1015/storage.$Stream$.stream"
            )
            .as_slice(),
        ),
        (
            STORAGE,
            "f0000000001",
            include_bytes!(
                "../../tests/fixtures/common-settings-native-1015/storage.f0000000001.stream"
            )
            .as_slice(),
        ),
    ] {
        assert_eq!(
            read_container_entry(container, name, 1024)
                .unwrap()
                .as_deref(),
            Some(expected)
        );
    }
}

#[test]
fn missing_name_is_distinct_from_invalid_container() {
    assert_eq!(read_container_entry(SCALAR, "absent", 1024).unwrap(), None);
    assert!(read_container_entry(&[], "$Stream$", 1024).is_err());
    assert!(read_container_entry(&SCALAR[..30], "$Stream$", 1024).is_err());
}

#[test]
fn bounds_are_checked_before_collecting_documents() {
    assert!(read_container_entry(STORAGE, "$Stream$", 1).is_err());
    assert!(document(SCALAR, 16, 0x200, 0).is_err());
}

#[test]
fn cyclic_chain_is_rejected() {
    let mut bytes = SCALAR.to_vec();
    bytes[18..26].copy_from_slice(b"00000400");
    bytes[36..44].copy_from_slice(b"00000010");
    assert!(document(&bytes, 16, 0x200, 2048).is_err());
}

#[test]
fn broken_directory_is_not_reported_as_known_absence() {
    let mut bytes = SCALAR.to_vec();
    bytes[18..26].copy_from_slice(b"0000000b");
    assert!(read_container_entry(&bytes, "absent", 1024).is_err());
    let mut bytes = SCALAR.to_vec();
    bytes[55] ^= 1;
    assert!(read_container_entry(&bytes, "absent", 1024).is_err());
}

#[test]
fn scalar_setting_remains_an_internal_value() {
    assert_eq!(
        decode_common_setting(SCALAR, 1024).unwrap(),
        DecodedCommonSetting::Value("{\"N\",17}".into())
    );
}

#[test]
fn multipart_storage_uses_the_existing_contents_codec() {
    let decoded = decode_common_setting(STORAGE, 1024).unwrap();
    let DecodedCommonSetting::ValueStorage(text) = decoded else {
        panic!("должно быть содержимое хранилища")
    };
    let expected = super::super::decode_stored_value_text(
        include_bytes!(
            "../../tests/fixtures/common-settings-native-1015/storage.f0000000001.stream"
        ),
        1024,
    )
    .unwrap();
    assert_eq!(text, expected);
    assert!(text.contains("probe"));
}

#[test]
fn missing_required_streams_are_errors() {
    let mut bytes = STORAGE.to_vec();
    let name: Vec<u8> = "f0000000001"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let start = bytes
        .windows(name.len())
        .position(|part| part == name)
        .unwrap();
    bytes[start] = b'g';
    assert!(decode_common_setting(&bytes, 1024).is_err());
    let mut bytes = SCALAR.to_vec();
    let name: Vec<u8> = "$Stream$"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let start = bytes
        .windows(name.len())
        .position(|part| part == name)
        .unwrap();
    bytes[start] = b'x';
    assert!(decode_common_setting(&bytes, 1024).is_err());
}
