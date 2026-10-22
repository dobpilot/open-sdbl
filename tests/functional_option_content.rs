mod support;

use open_sdbl::metadata::{
    DbNameEntry, DbNames, FunctionalOptionLocation, SchemaStorage, parse_config_descriptors,
    resolve_metadata,
};
use support::stored_deflate;

const BASE: &str = "tests/fixtures/functional-option-content-native";

#[test]
fn native_populated_content_preserves_location_and_both_flags() {
    for (id, expected) in [
        ("a11dfe2b-ef68-4701-9977-08fb0e680ee1", false),
        ("87c6ea4c-6187-4c3f-92ac-b691c1007e3e", true),
    ] {
        let bytes = std::fs::read(format!("{BASE}/resources/{id}.deflate")).unwrap();
        let descriptors = parse_config_descriptors(id, &bytes).unwrap();
        let d = descriptors
            .iter()
            .find(|d| d.object_guid.as_str() == id)
            .unwrap();
        assert_eq!(d.functional_option_privileged_get_mode, Some(expected));
        assert_eq!(
            d.functional_option_location_guid.as_ref().unwrap().as_str(),
            "9893e2d6-f3f8-4d73-bb06-19bf26d216ab"
        );
    }
}

#[test]
fn native_populated_options_keep_typed_location_after_source_removal() {
    let descriptors = std::fs::read_to_string(format!("{BASE}/resources/resources.txt"))
        .unwrap()
        .lines()
        .flat_map(|id| {
            let bytes = std::fs::read(format!("{BASE}/resources/{id}.deflate")).unwrap();
            parse_config_descriptors(id, &bytes).unwrap()
        })
        .collect();
    let source: open_sdbl::metadata::Guid = "9893e2d6-f3f8-4d73-bb06-19bf26d216ab".parse().unwrap();
    let snapshot = resolve_metadata(
        DbNames::from_entries(vec![DbNameEntry {
            guid: source.clone(),
            alias: "Const".into(),
            number: 1,
        }]),
        descriptors,
        SchemaStorage {
            tables: vec![],
            anomalies: vec![],
        },
        vec![],
    )
    .snapshot;
    for (id, expected) in [
        ("a11dfe2b-ef68-4701-9977-08fb0e680ee1", false),
        ("87c6ea4c-6187-4c3f-92ac-b691c1007e3e", true),
    ] {
        let d = snapshot
            .descriptors()
            .iter()
            .find(|d| d.object_guid.as_str() == id)
            .unwrap();
        assert_eq!(
            d.functional_option_location,
            Some(FunctionalOptionLocation::Constant(source.clone()))
        );
        assert_eq!(d.functional_option_privileged_get_mode, Some(expected));
    }
}

#[test]
fn retained_unf_populated_content_preserves_independent_fields() {
    let id = "19056365-9ff1-4733-876e-a7ebdbbe0557";
    let bytes = std::fs::read(format!("{BASE}/unf/{id}.serialized")).unwrap();
    let descriptors = parse_config_descriptors(id, &stored_deflate(&bytes)).unwrap();
    let d = descriptors
        .iter()
        .find(|d| d.object_guid.as_str() == id)
        .unwrap();
    assert_eq!(d.functional_option_privileged_get_mode, Some(true));
    assert_eq!(
        d.functional_option_location_guid.as_ref().unwrap().as_str(),
        "cc906f05-5ea6-43cb-a084-c4ee51ca4bd5"
    );
}

#[test]
fn malformed_populated_content_never_publishes_partial_option_fields() {
    let id = "87c6ea4c-6187-4c3f-92ac-b691c1007e3e";
    let text = std::fs::read_to_string(format!("{BASE}/resources/{id}.serialized")).unwrap();
    for bad in [
        text.replace("{0,2,", "{0,3,"),
        text.replace(
            "3ea29ea5-66f6-4e3b-8595-d8940db766a2",
            "00000000-0000-0000-0000-000000000000",
        ),
        text.replace(
            "157fa490-4ce9-11d4-9415-008048da11f9",
            "00000000-0000-0000-0000-000000000000",
        ),
        text.replace(
            "{1,a7177622-2cb3-5bfd-a0d7-1ae7a4a9f4d5}",
            "{1,00000000-0000-0000-0000-000000000000}",
        ),
        text.replace("},1},0}", "},9},0}"),
        text.replace(
            "{1,0,87c6ea4c-6187-4c3f-92ac-b691c1007e3e}",
            "{9,0,87c6ea4c-6187-4c3f-92ac-b691c1007e3e}",
        ),
        text.replace("},1},0}", "},1},9}"),
    ] {
        assert_ne!(text, bad);
        let ds = parse_config_descriptors(id, &stored_deflate(bad.as_bytes())).unwrap();
        let d = ds.iter().find(|d| d.object_guid.as_str() == id).unwrap();
        assert!(d.functional_option_privileged_get_mode.is_none());
        assert!(d.functional_option_location_guid.is_none());
    }
    assert!(
        parse_config_descriptors(id, &stored_deflate(&text.as_bytes()[..text.len() - 3])).is_err()
    );
}
