mod support;
use open_sdbl::metadata::{DbNames, SchemaStorage, parse_config_descriptors, resolve_metadata};

#[test]
fn unchanged_header19_enum_matches_native_ref_and_survives_snapshot_resolution() {
    let id = "1e057ae0-c258-4599-9816-0e609d98a3be";
    let bytes = include_bytes!(
        "fixtures/enum-header19-native/1e057ae0-c258-4599-9816-0e609d98a3be.deflate"
    );
    let descriptors = parse_config_descriptors(id, bytes).unwrap();
    let owner = descriptors
        .iter()
        .find(|d| d.object_guid.as_str() == id)
        .unwrap();
    assert_eq!(
        owner.object_reference_type.as_ref().map(|g| g.as_str()),
        Some("be2470cf-d875-47ce-af16-a604d1f8c90a")
    );
    assert_eq!(
        descriptors.iter().filter(|d| d.enumeration_value).count(),
        2
    );
    let snapshot = resolve_metadata(
        DbNames::from_entries(vec![]),
        descriptors,
        SchemaStorage {
            tables: vec![],
            anomalies: vec![],
        },
        vec![],
    )
    .snapshot;
    let owner = snapshot
        .descriptors()
        .iter()
        .find(|d| d.object_guid.as_str() == id)
        .unwrap();
    assert_eq!(
        owner.object_reference_type.as_ref().map(|g| g.as_str()),
        Some("be2470cf-d875-47ce-af16-a604d1f8c90a")
    );
}

#[test]
fn all_five_unchanged_buh_enums_match_their_native_ref_categories() {
    for (id, reference, values) in [
        (
            "fe9c408b-b39c-40b8-be5d-9061ba2198ec",
            "4678b32c-95d5-458a-9794-12a23bd6b37c",
            12,
        ),
        (
            "85e94a95-10a0-4116-8984-125a166bcf02",
            "89481115-d690-4ca9-b3c5-86ae6e46ec92",
            3,
        ),
        (
            "1e057ae0-c258-4599-9816-0e609d98a3be",
            "be2470cf-d875-47ce-af16-a604d1f8c90a",
            2,
        ),
        (
            "8f758f39-1f5a-4b1f-abe7-72ed99516ec1",
            "d49270ae-b525-4966-b9a4-a45a0ee5eae8",
            3,
        ),
        (
            "9cc43f9d-e693-42bb-8439-5754583bd7ac",
            "eff45316-caae-482e-8d93-59c0d152b28d",
            5,
        ),
    ] {
        let bytes =
            std::fs::read(format!("tests/fixtures/enum-header19-native/{id}.deflate")).unwrap();
        let descriptors = parse_config_descriptors(id, &bytes).unwrap();
        let owner = descriptors
            .iter()
            .find(|d| d.object_guid.as_str() == id)
            .unwrap();
        assert_eq!(
            owner.object_reference_type.as_ref().map(|g| g.as_str()),
            Some(reference),
            "{id}"
        );
        assert_eq!(
            descriptors.iter().filter(|d| d.enumeration_value).count(),
            values,
            "{id}"
        );
    }
}

#[test]
fn modern_enum_and_report_keep_distinct_native_contracts() {
    let id = "c143f002-c8de-45ef-8272-ca10ec452db4";
    let ds = parse_config_descriptors(
        id,
        include_bytes!("fixtures/enum-header19-native/modern-header20.deflate"),
    )
    .unwrap();
    assert_eq!(
        ds.iter()
            .find(|d| d.object_guid.as_str() == id)
            .unwrap()
            .object_reference_type
            .as_ref()
            .unwrap()
            .as_str(),
        "a9919a7a-f80a-419e-9f46-969dd298b58d"
    );
    let ds = parse_config_descriptors(
        "8bac013d-e4dc-43c2-980d-6190972b6678",
        include_bytes!("fixtures/report-object/ЗамерОтчета.deflate"),
    )
    .unwrap();
    let report = ds.iter().find(|d| d.name == "ЗамерОтчета").unwrap();
    assert!(report.object_reference_type.is_none());
    assert!(report.report_object_type.is_some());
}

#[test]
fn malformed_legacy_enum_profiles_never_publish_manager_as_reference() {
    let id = "1e057ae0-c258-4599-9816-0e609d98a3be";
    let text = include_str!(
        "fixtures/enum-header19-native/1e057ae0-c258-4599-9816-0e609d98a3be.serialized"
    );
    for bad in [
        text.replace(
            "be2470cf-d875-47ce-af16-a604d1f8c90a",
            "00000000-0000-0000-0000-000000000000",
        ),
        text.replace("6e36c95c-5a42-47ed-8c43-439ecb084607", "invalid-guid"),
        text.replace(
            "bee0a08c-07eb-40c0-8544-5c364c171465,2,",
            "bee0a08c-07eb-40c0-8544-5c364c171465,3,",
        ),
        text.replace(
            "{33f2e54b-37ce-4a7a-a569-b648d7aa4634,0}",
            "{33f2e54b-37ce-4a7a-a569-b648d7aa4634,1}",
        ),
        text.replace(
            "8069bd33-647d-4627-a6df-13746c25e84e",
            "45e5b183-a2df-43d4-8c54-a7801e93c65e",
        ),
        text.replace(
            "{0,0,1e057ae0-c258-4599-9816-0e609d98a3be}",
            "{7,0,1e057ae0-c258-4599-9816-0e609d98a3be}",
        ),
    ] {
        assert_ne!(text, bad);
        let ds = parse_config_descriptors(id, &support::stored_deflate(bad.as_bytes())).unwrap();
        assert!(ds.iter().all(|d| d.object_reference_type.is_none()));
    }
}
