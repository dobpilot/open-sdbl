mod support;
use open_sdbl::metadata::{
    MetadataErrorKind, decode_stored_value, decode_stored_value_text, parse_serialized,
};
use std::borrow::Cow;
const TEXT: &str = r#"{2,{"S","Текст ""кавычки"""},{"B",1}}"#;
fn frame(body: &[u8]) -> Vec<u8> {
    let mut b = vec![1, 1];
    b.extend_from_slice(&(body.len() as u64).to_le_bytes());
    b.extend_from_slice(body);
    b
}
#[test]
fn plain_text_borrows_and_preserves_the_exact_serialized_payload_without_bom() {
    for bom in [false, true] {
        let mut body = Vec::new();
        if bom {
            body.extend_from_slice(&[0xef, 0xbb, 0xbf]);
        }
        body.extend_from_slice(TEXT.as_bytes());
        let bytes = frame(&body);
        let text = decode_stored_value_text(&bytes, body.len()).unwrap();
        assert!(matches!(text, Cow::Borrowed(_)));
        assert_eq!(text, TEXT);
        assert_eq!(
            text.as_ptr(),
            bytes[10 + if bom { 3 } else { 0 }..].as_ptr()
        );
        assert_eq!(
            decode_stored_value(&bytes, body.len()).unwrap(),
            parse_serialized(text.as_bytes()).unwrap()
        );
        assert!(decode_stored_value_text(&bytes, body.len() - 1).is_err());
        assert!(decode_stored_value(&bytes, body.len() - 1).is_err());
    }
}
#[test]
fn compressed_text_owns_the_same_payload_and_obeys_the_decoded_byte_limit() {
    for bom in [false, true] {
        let mut body = Vec::new();
        if bom {
            body.extend_from_slice(&[0xef, 0xbb, 0xbf]);
        }
        body.extend_from_slice(TEXT.as_bytes());
        let bytes = frame(&support::stored_deflate(&body));
        let text = decode_stored_value_text(&bytes, body.len()).unwrap();
        assert!(matches!(text, Cow::Owned(_)));
        assert_eq!(text, TEXT);
        assert_eq!(
            decode_stored_value(&bytes, body.len()).unwrap(),
            parse_serialized(text.as_bytes()).unwrap()
        );
        assert!(decode_stored_value_text(&bytes, body.len() - 1).is_err());
        assert!(decode_stored_value(&bytes, body.len() - 1).is_err());
    }
}
#[test]
fn malformed_framing_utf8_and_external_reference_are_not_guessed() {
    for bytes in [
        vec![0; 9],
        frame(b"not a stream"),
        b"STORHDR000000000000000000000000000000000000".to_vec(),
    ] {
        assert!(decode_stored_value_text(&bytes, 100).is_err());
        assert!(decode_stored_value(&bytes, 100).is_err());
    }
    let invalid = frame(&[b'{', 0xff, b'}']);
    let error = decode_stored_value_text(&invalid, 100).unwrap_err();
    assert_eq!(error.kind(), MetadataErrorKind::Utf8);
    let compressed = frame(&support::stored_deflate(&[b'{', 0xff, b'}']));
    assert_eq!(
        decode_stored_value_text(&compressed, 100)
            .unwrap_err()
            .kind(),
        MetadataErrorKind::Utf8
    );
}
#[test]
fn text_extraction_does_not_parse_or_rewrite_the_brace_grammar() {
    let bytes = frame(b"{broken");
    assert_eq!(decode_stored_value_text(&bytes, 100).unwrap(), "{broken");
    assert!(decode_stored_value(&bytes, 100).is_err());
}

#[test]
fn distinct_native_structure_storage_envelope_is_not_guessed() {
    for bytes in [
        include_bytes!("fixtures/stored-value-text-native-1013/db-storage-0.bin").as_slice(),
        include_bytes!("fixtures/stored-value-text-native-1013/db-storage-1.bin").as_slice(),
    ] {
        assert!(decode_stored_value_text(bytes, 1024).is_err());
        assert!(decode_stored_value(bytes, 1024).is_err());
    }
}
