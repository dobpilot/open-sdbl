use super::*;

const SCALAR: &[u8] =
    include_bytes!("../../tests/fixtures/common-settings-native-1015/scalar.settingsdata");
const STORAGE: &[u8] =
    include_bytes!("../../tests/fixtures/common-settings-native-1015/storage.settingsdata");

#[test]
fn native_module_source_preserves_the_exact_text() {
    let cf = include_bytes!("../../tests/fixtures/data-processor-header2/control.cf");
    let compressed = read_container_entry(cf, "4507a92e-abb6-4329-88b2-3429bc694bc6.0", 8_000_000)
        .unwrap()
        .unwrap();
    let container = super::super::inflate_raw_deflate_bounded(&compressed, 1_000_000).unwrap();
    let source = decode_module_source(&container, 1_000_000).unwrap();
    let raw = read_container_entry(&container, "text", 1_000_000)
        .unwrap()
        .unwrap();
    assert_eq!(
        source.as_bytes(),
        raw.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap()
    );
    assert!(source.contains("КонецФункции\r\n"));
    assert!(decode_module_source(&container, 100).is_err());
    assert!(decode_module_source(SCALAR, 1_000_000).is_err());
}

#[test]
fn module_text_accepts_empty_and_valid_unicode_but_rejects_binary() {
    assert_eq!(decode_module_text(b"\xef\xbb\xbf", 3).unwrap(), "");
    assert_eq!(decode_module_text(b"\xff\xfe\x16\x04", 2).unwrap(), "Ж");
    assert!(decode_module_text(b"\xff\xfe\x16\x04", 1).is_err());
    assert!(decode_module_text(b"\xff\xfe\x00\xd8", 100).is_err());
    assert!(decode_module_text(b"\xff\xfea", 100).is_err());
    assert!(decode_module_text(b"\xff\xfe\x00\x00", 100).is_err());
    assert!(decode_module_text(b"bad\0text", 100).is_err());
    assert!(decode_module_text(b"\xff", 100).is_err());
}

#[test]
fn native_managed_form_modules_match_designer_exports() {
    let cf = include_bytes!("../../tests/fixtures/data-processor-header2/control.cf");
    for (name, expected) in [
        (
            "ea0843f4-60fb-4d44-abf4-0021867c636b.0",
            include_bytes!("../../tests/fixtures/data-processor-header2/form-profile49.bsl")
                .as_slice(),
        ),
        (
            "efb6b3b6-d57b-469f-a7c2-f4e0308d0c7f.0",
            include_bytes!("../../tests/fixtures/data-processor-header2/form-profile50.bsl")
                .as_slice(),
        ),
    ] {
        let compressed = read_container_entry(cf, name, 8_000_000).unwrap().unwrap();
        let serialized = super::super::inflate_raw_deflate_bounded(&compressed, 1_000_000).unwrap();
        let actual = decode_managed_form_module(&serialized, 1_000_000).unwrap();
        assert_eq!(
            actual.as_bytes(),
            expected.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap()
        );
        assert!(decode_managed_form_module(&serialized, 100).is_err());
    }
}

#[test]
fn managed_form_modules_reject_unknown_or_malformed_profiles() {
    let header = format!("{{49{}}}", ",0".repeat(49));
    let valid = format!("{{4,{header},\"module\",{{0}},{{0}},{{0}},{{0}},{{0}},0,0}}");
    assert_eq!(
        decode_managed_form_module(valid.as_bytes(), 1000).unwrap(),
        "module"
    );
    for invalid in [
        valid.replacen("{49,", "{48,", 1),
        valid.replacen("{49,0,", "{49,", 1),
        valid.replacen("{4,", "{3,", 1),
        valid.replace("\"module\"", "0"),
        valid.replacen(",0,0}", ",0}", 1),
        valid.replace("module", "bad\0text"),
    ] {
        assert!(decode_managed_form_module(invalid.as_bytes(), 1000).is_err());
    }
}

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
