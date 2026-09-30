//! Legacy-заголовки расширения, сопоставленные с нативной XML-выгрузкой.

use open_sdbl::metadata::{ObjectBelonging, parse_extension_objects};
use std::path::PathBuf;

#[test]
fn every_missing_buh_header_matches_native_xml_identity_name_and_belonging() {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/extension/legacy-buh");
    let rows = std::fs::read_to_string(root.join("native-objects.tsv")).unwrap();
    assert_eq!(rows.lines().count(), 37);
    for row in rows.lines() {
        let fields: Vec<_> = row.split('\t').collect();
        let bytes = std::fs::read(root.join(format!("{}.serialized", fields[0]))).unwrap();
        let objects = parse_extension_objects(&bytes).unwrap();
        let object = objects
            .iter()
            .find(|object| object.id.as_str() == fields[0])
            .unwrap();
        assert_eq!(object.name, fields[1]);
        assert_eq!(fields[2], "Adopted");
        assert_eq!(object.belonging, Some(ObjectBelonging::Adopted));
        assert_eq!(object.extends, None);
    }
}

#[test]
fn unsupported_legacy_tail_keeps_identity_but_does_not_infer_a_link() {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/extension/legacy-buh");
    let text =
        std::fs::read_to_string(root.join("f841cbe0-2659-469c-a3ee-7184a0c6811f.serialized"))
            .unwrap();
    for tail in ["9,0", "1,2", "1,1,00000000-0000-0000-0000-000000000000"] {
        let changed = text.replace("\"\",1,0", &format!("\"\",{tail}"));
        assert_ne!(changed, text);
        let objects = parse_extension_objects(changed.as_bytes()).unwrap();
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].belonging, None);
        assert_eq!(objects[0].extends, None);
    }
}
