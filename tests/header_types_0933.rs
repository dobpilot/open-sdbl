mod support;
use open_sdbl::metadata::{
    ConfigTypeIdentity, Guid, parse_config_descriptor, parse_config_header_bounded,
};

#[test]
fn generated_types_and_sections_agree_with_native_config_corpora() {
    for base in ["demo", "buh", "unf"] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(base)
            .join("config.pack");
        let pack = std::fs::read(path).unwrap();
        let mut offset = 0;
        let mut checked = 0;
        while offset < pack.len() {
            let end = offset
                + pack[offset..]
                    .iter()
                    .position(|byte| *byte == b'\n')
                    .unwrap();
            let header = std::str::from_utf8(&pack[offset..end]).unwrap();
            let (name, length) = header.split_once('\t').unwrap();
            let length: usize = length.parse().unwrap();
            let bytes = &pack[end + 1..end + 1 + length];
            offset = end + 1 + length;
            if name.parse::<Guid>().is_err() {
                continue;
            }
            let expected = parse_config_descriptor(name, bytes);
            if let Ok(expected) = expected {
                let actual = parse_config_header_bounded(name, bytes, 32 * 1024 * 1024).unwrap();
                assert_eq!(
                    actual
                        .as_ref()
                        .map(|own| (&own.object_guid, own.name.as_str())),
                    expected
                        .as_ref()
                        .map(|own| (&own.object_guid, own.name.as_str())),
                    "{base}: {name}"
                );
                if let (Some(header), Some(owner)) = (&actual, &expected) {
                    match (&header.reference_type, &owner.object_reference_type) {
                        (ConfigTypeIdentity::Known(actual), Some(expected)) => {
                            assert_eq!(actual, expected, "{base}:{name}")
                        }
                        (ConfigTypeIdentity::Absent, Some(expected)) => assert!(expected.is_nil()),
                        (ConfigTypeIdentity::Unknown, _) => {}
                        (actual, expected) => panic!("{base}:{name}: {actual:?} != {expected:?}"),
                    }
                    assert_eq!(
                        header.synonyms, owner.synonyms,
                        "{base}:{name}: own synonyms"
                    );
                    if let Some(expected) = &owner.business_process_route_point_reference_type {
                        assert_eq!(
                            header.route_point_reference_type,
                            ConfigTypeIdentity::Known(expected.clone()),
                            "{base}:{name}: route-point type"
                        );
                    }
                    let descriptors =
                        open_sdbl::metadata::parse_config_descriptors(name, bytes).unwrap();
                    let expected_members = descriptors
                        .iter()
                        .map(|d| d.object_guid.as_str())
                        .collect::<std::collections::BTreeSet<_>>();
                    let actual_members = header
                        .member_guids
                        .iter()
                        .map(|id| id.as_str())
                        .collect::<std::collections::BTreeSet<_>>();
                    assert_eq!(
                        actual_members, expected_members,
                        "{base}:{name}: recognized members"
                    );
                    let expected_values = descriptors
                        .iter()
                        .filter(|d| d.enumeration_value)
                        .collect::<Vec<_>>();
                    if !expected_values.is_empty() {
                        let values = header
                            .enumeration_values
                            .as_ref()
                            .unwrap_or_else(|| panic!("{base}:{name}: enum values missing"));
                        assert_eq!(
                            values.len(),
                            expected_values.len(),
                            "{base}:{name}: enum size"
                        );
                        for value in values {
                            let expected = expected_values
                                .iter()
                                .find(|d| d.object_guid == value.object_guid)
                                .unwrap();
                            assert_eq!(value.name, expected.name);
                            assert_eq!(value.synonyms, expected.synonyms);
                        }
                    }
                    let object = owner
                        .data_processor_object_type
                        .as_ref()
                        .or(owner.report_object_type.as_ref())
                        .or(owner.business_process_object_type.as_ref());
                    if let Some(expected) = object {
                        assert_eq!(
                            header.object_type,
                            ConfigTypeIdentity::Known(expected.clone()),
                            "{base}:{name}"
                        );
                    }
                    let sections = owner
                        .document_tabular_sections
                        .as_ref()
                        .or(owner.business_process_tabular_sections.as_ref())
                        .or(owner.data_processor_tabular_sections.as_ref())
                        .or(owner.report_tabular_sections.as_ref());
                    if let Some(expected) = sections {
                        let actual = header
                            .sections
                            .as_ref()
                            .unwrap_or_else(|| panic!("{base}:{name}: sections missing"));
                        assert_eq!(
                            actual.iter().map(|s| &s.object_guid).collect::<Vec<_>>(),
                            expected.iter().collect::<Vec<_>>(),
                            "{base}:{name}"
                        );
                        let descriptors =
                            open_sdbl::metadata::parse_config_descriptors(name, bytes).unwrap();
                        for part in actual {
                            assert_eq!(
                                part.name,
                                descriptors
                                    .iter()
                                    .find(|d| d.object_guid == part.object_guid)
                                    .unwrap()
                                    .name
                            );
                        }
                    }
                }
                checked += 1;
            }
        }
        eprintln!("{base}: {checked} native headers");
        assert!(checked >= 12, "{base}: {checked}");
    }
}

#[test]
fn unknown_and_measured_empty_type_positions_remain_distinct() {
    let own = "11111111-1111-4111-8111-111111111111";
    let nil = "00000000-0000-0000-0000-000000000000";
    let other = "22222222-2222-4222-8222-222222222222";
    let body = format!(
        "{{1,{{17,{nil},{other},{{0,{{3,{{1,0,{own}}},\"Owner\",{{0}},\"\",0,0,{nil},0}}}},0,0,0,{other},{other},{other},0,0}}}}"
    );
    let header = parse_config_header_bounded(own, &support::stored_deflate(body.as_bytes()), 4096)
        .unwrap()
        .unwrap();
    assert_eq!(header.object_type, ConfigTypeIdentity::Absent);
    assert_eq!(header.reference_type, ConfigTypeIdentity::Unknown);
    let unknown = format!("{{{{1,0,{own}}},\"Owner\",{{0}}}}");
    let header =
        parse_config_header_bounded(own, &support::stored_deflate(unknown.as_bytes()), 4096)
            .unwrap()
            .unwrap();
    assert_eq!(header.object_type, ConfigTypeIdentity::Unknown);
    assert!(header.sections.is_none());
}

#[test]
fn a_class_number_alone_does_not_publish_a_route_point_type() {
    let own = "11111111-1111-4111-8111-111111111111";
    let id = "22222222-2222-4222-8222-222222222222";
    let nil = "00000000-0000-0000-0000-000000000000";
    let body = format!(
        "{{1,{{30,{id},{id},{id},{id},{id},{id},{id},{id},{{0,{{3,{{1,0,{own}}},\"Owner\",{{0}},\"\",0,0,{nil},0}}}},0,0,0,{id}}}}}"
    );
    let header = parse_config_header_bounded(own, &support::stored_deflate(body.as_bytes()), 4096)
        .unwrap()
        .unwrap();
    assert_eq!(
        header.route_point_reference_type,
        ConfigTypeIdentity::Unknown
    );
}
