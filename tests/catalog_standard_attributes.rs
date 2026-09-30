mod support;

use open_sdbl::metadata::{
    CatalogCodeType, CatalogStandardAttributes, inflate_raw_deflate, parse_config_descriptors,
};
use std::collections::HashMap;
use support::stored_deflate;

const RESOURCE: &str = "4bb0f7c3-62f3-4352-9bc8-e243dd18fe4a";
const VERSIONS: &[u8] = include_bytes!("fixtures/catalog-standard-attributes/versions.deflate");

#[test]
fn acquired_unf_catalog_settings_match_uuid_bound_native_xml() {
    let expected = include_str!("fixtures/catalog-standard-attributes/unf-native.tsv")
        .lines()
        .map(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            (fields[0], fields)
        })
        .collect::<HashMap<_, _>>();
    let pack = include_bytes!("fixtures/unf/config.pack");
    let mut offset = 0;
    let mut checked = 0;
    let mut numbers = 0;
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
            let code_type = match fields[2] {
                "String" => CatalogCodeType::String,
                "Number" => CatalogCodeType::Number,
                _ => panic!("unknown native type"),
            };
            assert_eq!(
                owner.catalog_standard_attributes,
                Some(CatalogStandardAttributes {
                    code_type,
                    code_length: fields[3].parse().unwrap(),
                    description_length: fields[4].parse().unwrap()
                }),
                "{resource}"
            );
            assert!(
                descriptors
                    .iter()
                    .filter(|descriptor| descriptor.object_guid.to_string() != resource)
                    .all(|descriptor| descriptor.catalog_standard_attributes.is_none())
            );
            checked += 1;
            numbers += usize::from(code_type == CatalogCodeType::Number);
        }
        offset += length;
    }
    assert_eq!(checked, 296);
    assert_eq!(numbers, 6);
}

#[test]
fn numeric_code_and_zero_description_length_are_acquired_without_sql() {
    let descriptors = parse_config_descriptors(RESOURCE, VERSIONS).unwrap();
    let owner = descriptors
        .iter()
        .find(|descriptor| descriptor.object_guid.to_string() == RESOURCE)
        .unwrap();
    assert_eq!(
        owner.catalog_standard_attributes,
        Some(CatalogStandardAttributes {
            code_type: CatalogCodeType::Number,
            code_length: 9,
            description_length: 0
        })
    );
}

#[test]
fn unknown_catalog_settings_never_become_zero_lengths() {
    let text = String::from_utf8(inflate_raw_deflate(VERSIONS).unwrap()).unwrap();
    let cutoff = text.find(",9,0,0,0,a43e9de0").unwrap();
    let truncated = format!("{}}}}}", &text[..cutoff]);
    let descriptors =
        parse_config_descriptors(RESOURCE, &stored_deflate(truncated.as_bytes())).unwrap();
    assert!(
        descriptors
            .iter()
            .all(|descriptor| descriptor.catalog_standard_attributes.is_none())
    );
    for (before, after) in [
        (",9,0,0,0,a43e9de0", ",9,8,0,0,a43e9de0"),
        (",9,0,0,0,a43e9de0", ",9,,0,0,a43e9de0"),
        (",9,0,0,0,a43e9de0", ",9,0,,0,a43e9de0"),
        (",9,0,0,0,a43e9de0", ",\"9\",0,0,0,a43e9de0"),
        ("{57,", "{58,"),
    ] {
        assert!(text.contains(before));
        let changed = text.replacen(before, after, 1);
        let descriptors =
            parse_config_descriptors(RESOURCE, &stored_deflate(changed.as_bytes())).unwrap();
        assert!(
            descriptors
                .iter()
                .all(|descriptor| descriptor.catalog_standard_attributes.is_none()),
            "{after}"
        );
    }
}
