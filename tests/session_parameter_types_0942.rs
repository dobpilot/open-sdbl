use open_sdbl::metadata::parse_config_resource_bounded;

const OWNER: &str = "53ee3eb6-d2d1-4fb3-8a78-1fa4543850a1";

#[test]
fn native_uuid_session_parameter_matches_xml_and_edt() {
    let parsed = parse_config_resource_bounded(
        OWNER,
        include_bytes!("fixtures/session-parameter-0942/config.deflate"),
        1024 * 1024,
    )
    .unwrap();
    let owner = parsed
        .descriptors
        .iter()
        .find(|d| d.object_guid.as_str() == OWNER)
        .unwrap();
    let types = owner.declared_types.as_ref().unwrap();
    assert_eq!(types.len(), 1);
    assert_eq!(types[0].tag, "#");
    assert_eq!(
        types[0].reference.as_ref().unwrap().as_str(),
        "fc01b5df-97fe-449b-83d4-218a090e681e"
    );
    assert!(types[0].qualifiers.is_empty());
    assert!(
        include_str!("fixtures/session-parameter-0942/SessionParameter.xml")
            .contains("<v8:Type>v8:UUID</v8:Type>")
    );
    assert!(
        include_str!("fixtures/session-parameter-0942/SessionParameter.mdo")
            .contains("<types>UUID</types>")
    );
}

#[allow(clippy::duplicate_mod)]
#[path = "support/mod.rs"]
mod support;

fn native_text() -> String {
    String::from_utf8(
        open_sdbl::metadata::inflate_raw_deflate(include_bytes!(
            "fixtures/session-parameter-0942/config.deflate"
        ))
        .unwrap(),
    )
    .unwrap()
}

fn declared(text: &str) -> Option<Vec<open_sdbl::metadata::ConfigDeclaredType>> {
    open_sdbl::metadata::parse_config_descriptors(OWNER, &support::stored_deflate(text.as_bytes()))
        .unwrap()
        .into_iter()
        .find(|d| d.object_guid.as_str() == OWNER)
        .and_then(|d| d.declared_types)
}

#[test]
fn complete_patterns_preserve_unknown_reference_ids_and_primitive_qualifiers() {
    let text = native_text().replace(
        r##"{"#",fc01b5df-97fe-449b-83d4-218a090e681e}"##,
        r##"{"#",11111111-2222-4333-8444-555555555555},{"N",12,3,0},{"S",42,1}"##,
    );
    let types = declared(&text).unwrap();
    assert_eq!(types.len(), 3);
    assert_eq!(
        types[0].reference.as_ref().unwrap().as_str(),
        "11111111-2222-4333-8444-555555555555"
    );
    assert_eq!(types[1].tag, "N");
    assert_eq!(types[1].qualifiers.len(), 3);
    assert_eq!(types[2].tag, "S");
    assert_eq!(types[2].qualifiers.len(), 2);
}

#[test]
fn an_unknown_or_malformed_pattern_member_does_not_publish_a_prefix() {
    for member in [r##"{"X"}"##, r##"{"#"}"##, r##"{"#",bad}"##] {
        let text = native_text().replace(
            r##"{"#",fc01b5df-97fe-449b-83d4-218a090e681e}"##,
            &format!(r##"{{"B"}},{member}"##),
        );
        assert_eq!(declared(&text), None, "{member}");
    }
}

#[test]
fn unmeasured_envelopes_do_not_borrow_a_nested_pattern() {
    let text = native_text();
    for modified in [
        text.replacen("{1,", "{9,", 1),
        text.replacen("{2,", "{9,", 1),
        text.replacen("{3,", "{9,", 1),
        text.replacen("{1,0,", "{2,0,", 1),
        text.replacen("},0}", "},1}", 1),
        text.replacen("},0}", "},0,0}", 1),
    ] {
        assert_ne!(modified, text);
        assert_eq!(declared(&modified), None);
    }
    assert!(
        open_sdbl::metadata::parse_config_resource_bounded(
            "11111111-2222-4333-8444-555555555555",
            include_bytes!("fixtures/session-parameter-0942/config.deflate"),
            1024 * 1024,
        )
        .unwrap()
        .descriptors
        .iter()
        .all(|d| d.declared_types.is_none())
    );
}
