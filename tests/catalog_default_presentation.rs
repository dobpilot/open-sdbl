mod support;

use open_sdbl::metadata::{
    CatalogDefaultPresentation, inflate_raw_deflate, parse_config_descriptors,
};
use std::collections::HashMap;
use support::stored_deflate;

const RESOURCE: &str = "4bb0f7c3-62f3-4352-9bc8-e243dd18fe4a";
const VERSIONS: &[u8] = include_bytes!("fixtures/catalog-standard-attributes/versions.deflate");

#[test]
fn acquired_unf_presentation_matches_both_native_xml_variants() {
    let expected = include_str!("fixtures/catalog-standard-attributes/unf-presentation-native.tsv")
        .lines()
        .map(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            (fields[0], fields)
        })
        .collect::<HashMap<_, _>>();
    let pack = include_bytes!("fixtures/unf/config.pack");
    let mut offset = 0;
    let mut code_count = 0;
    let mut description_count = 0;
    while offset < pack.len() {
        let end = offset
            + pack[offset..]
                .iter()
                .position(|byte| *byte == b'\n')
                .unwrap();
        let (resource, length) = std::str::from_utf8(&pack[offset..end])
            .unwrap()
            .split_once('\t')
            .unwrap();
        let length: usize = length.parse().unwrap();
        offset = end + 1;
        if let Some(fields) = expected.get(resource) {
            let descriptors =
                parse_config_descriptors(resource, &pack[offset..offset + length]).unwrap();
            let owner = descriptors
                .iter()
                .find(|descriptor| descriptor.object_guid.to_string() == resource)
                .unwrap();
            assert_eq!(owner.name, fields[1]);
            let presentation = match fields[2] {
                "AsCode" => {
                    code_count += 1;
                    CatalogDefaultPresentation::AsCode
                }
                "AsDescription" => {
                    description_count += 1;
                    CatalogDefaultPresentation::AsDescription
                }
                _ => panic!("unknown native presentation"),
            };
            assert_eq!(
                owner.catalog_default_presentation,
                Some(presentation),
                "{resource}"
            );
            assert!(
                descriptors
                    .iter()
                    .filter(|descriptor| descriptor.object_guid.to_string() != resource)
                    .all(|descriptor| descriptor.catalog_default_presentation.is_none())
            );
        }
        offset += length;
    }
    assert_eq!(code_count, 26);
    assert_eq!(description_count, 270);
}

#[test]
fn unknown_presentation_never_falls_back_to_known_code_settings() {
    let text = String::from_utf8(inflate_raw_deflate(VERSIONS).unwrap()).unwrap();
    for replacement in [
        ",9,0,0,2,a43e9de0",
        ",9,0,0,,a43e9de0",
        ",9,0,0,\"0\",a43e9de0",
    ] {
        let changed = text.replacen(",9,0,0,0,a43e9de0", replacement, 1);
        assert_ne!(changed, text);
        let descriptors =
            parse_config_descriptors(RESOURCE, &stored_deflate(changed.as_bytes())).unwrap();
        let owner = descriptors
            .iter()
            .find(|descriptor| descriptor.object_guid.to_string() == RESOURCE)
            .unwrap();
        assert!(owner.catalog_standard_attributes.is_some());
        assert!(owner.catalog_default_presentation.is_none());
    }
    let cutoff = text.find(",0,a43e9de0").unwrap();
    let truncated = format!("{}}}}}", &text[..cutoff]);
    let descriptors =
        parse_config_descriptors(RESOURCE, &stored_deflate(truncated.as_bytes())).unwrap();
    let owner = descriptors
        .iter()
        .find(|descriptor| descriptor.object_guid.to_string() == RESOURCE)
        .unwrap();
    assert!(owner.catalog_standard_attributes.is_some());
    assert!(owner.catalog_default_presentation.is_none());
}

#[test]
fn presentation_is_acquired_independently_of_code_type_and_lengths() {
    let text = String::from_utf8(inflate_raw_deflate(VERSIONS).unwrap()).unwrap();
    let descriptors = parse_config_descriptors(RESOURCE, VERSIONS).unwrap();
    let owner = descriptors
        .iter()
        .find(|descriptor| descriptor.object_guid.to_string() == RESOURCE)
        .unwrap();
    assert_eq!(
        owner.catalog_default_presentation,
        Some(CatalogDefaultPresentation::AsCode)
    );
    let changed = text.replacen(",9,0,0,0,a43e9de0", ",9,8,0,0,a43e9de0", 1);
    let descriptors =
        parse_config_descriptors(RESOURCE, &stored_deflate(changed.as_bytes())).unwrap();
    let owner = descriptors
        .iter()
        .find(|descriptor| descriptor.object_guid.to_string() == RESOURCE)
        .unwrap();
    assert!(owner.catalog_standard_attributes.is_none());
    assert_eq!(
        owner.catalog_default_presentation,
        Some(CatalogDefaultPresentation::AsCode)
    );
}
