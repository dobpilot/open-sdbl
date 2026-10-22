mod support;
use open_sdbl::metadata::{
    ConfigTypeIdentity, parse_config_descriptor, parse_config_header_bounded,
};
const OWNER: &str = "51c403a3-e062-4f0d-b39c-11989c0ffe2b";
const TYPE: &str = "4054e098-bf79-436d-8a4b-4e42d8f00f42";
const NATIVE: &[u8] = include_bytes!("fixtures/defined-type-header-0937/basis.deflate");

#[test]
fn native_defined_type_identity_matches_full_descriptor() {
    let descriptor = parse_config_descriptor(OWNER, NATIVE).unwrap().unwrap();
    assert_eq!(descriptor.defined_type_id.as_ref().unwrap().as_str(), TYPE);
    let header = parse_config_header_bounded(OWNER, NATIVE, 1024 * 1024)
        .unwrap()
        .unwrap();
    assert_eq!(
        header.defined_type,
        ConfigTypeIdentity::Known(TYPE.parse().unwrap())
    );
}

fn synthetic_header(text: &str) -> open_sdbl::metadata::ConfigHeader {
    parse_config_header_bounded(
        OWNER,
        &support::stored_deflate(text.as_bytes()),
        1024 * 1024,
    )
    .unwrap()
    .unwrap()
}

#[test]
fn synthetic_unknown_wrappers_and_invalid_guids_do_not_invent_identity() {
    use open_sdbl::metadata::inflate_raw_deflate;
    let text = String::from_utf8(inflate_raw_deflate(NATIVE).unwrap()).unwrap();
    for text in [
        text.replacen("{0,4054", "{999,4054", 1),
        text.replacen("{0,4054", "{0,extra,4054", 1),
        text.replacen(TYPE, "not-a-guid", 1),
    ] {
        assert_eq!(
            synthetic_header(&text).defined_type,
            ConfigTypeIdentity::Unknown
        );
    }
    let text = text.replacen(TYPE, "00000000-0000-0000-0000-000000000000", 1);
    assert_eq!(
        synthetic_header(&text).defined_type,
        ConfigTypeIdentity::Absent
    );
    let text = "{1,{0,4054e098-bf79-436d-8a4b-4e42d8f00f42";
    assert!(
        parse_config_header_bounded(
            OWNER,
            &support::stored_deflate(text.as_bytes()),
            1024 * 1024
        )
        .is_err()
    );
    assert!(parse_config_header_bounded(OWNER, NATIVE, 1).is_err());
    assert!(
        parse_config_header_bounded("11111111-1111-4111-8111-111111111111", NATIVE, 1024 * 1024)
            .unwrap()
            .is_none()
    );
}

#[test]
fn native_unknown_defined_dependency_survives_snapshot_resolution() {
    use open_sdbl::metadata::{parse_config_descriptors, resolve_metadata};
    let event = parse_config_descriptors(
        "571e1bd9-0f64-4a05-a276-dba4fe242ad0",
        include_bytes!("fixtures/defined-type-header-0937/event.deflate"),
    )
    .unwrap();
    let attribute = event
        .iter()
        .find(|d| d.name == "ДокументОснование")
        .unwrap();
    assert!(
        attribute
            .reference_types
            .iter()
            .any(|id| id.as_str() == TYPE)
    );
    let attribute_id = attribute.object_guid.clone();
    let baseline = support::snapshot();
    let resolved = resolve_metadata(
        baseline.db_names().clone(),
        event.clone(),
        baseline.schema().clone(),
        baseline.live_tables().to_vec(),
    );
    let attribute = resolved
        .snapshot
        .descriptors()
        .iter()
        .find(|d| d.object_guid == attribute_id)
        .unwrap();
    assert!(
        attribute
            .reference_types
            .iter()
            .any(|id| id.as_str() == TYPE)
    );
    assert!(
        attribute
            .declared_types
            .as_ref()
            .unwrap()
            .iter()
            .any(|ty| ty.reference.as_ref().is_some_and(|id| id.as_str() == TYPE))
    );
    let mut descriptors = event;
    descriptors.extend(parse_config_descriptors(OWNER, NATIVE).unwrap());
    let resolved = resolve_metadata(
        baseline.db_names().clone(),
        descriptors,
        baseline.schema().clone(),
        baseline.live_tables().to_vec(),
    );
    let attribute = resolved
        .snapshot
        .descriptors()
        .iter()
        .find(|d| d.object_guid == attribute_id)
        .unwrap();
    assert_eq!(attribute.declared_types.as_ref().unwrap().len(), 56);
    assert!(
        !attribute
            .reference_types
            .iter()
            .any(|id| id.as_str() == TYPE)
    );
}
