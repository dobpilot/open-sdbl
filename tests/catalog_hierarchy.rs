mod support;

use open_sdbl::metadata::{ConfigDescriptor, parse_config_descriptors};
use std::collections::HashMap;
use support::stored_deflate;

const FALSE: &str = include_str!("fixtures/catalog-hierarchy/false.serialized");
const FALSE_GUID: &str = include_str!("fixtures/catalog-hierarchy/false.guid");

fn owner(resource: &str, text: &str) -> ConfigDescriptor {
    parse_config_descriptors(resource.trim(), &stored_deflate(text.as_bytes()))
        .unwrap()
        .into_iter()
        .find(|descriptor| descriptor.object_guid.as_str() == resource.trim())
        .unwrap()
}

#[test]
fn hierarchy_matches_all_uuid_bound_native_xml_flags() {
    let expected = include_str!("fixtures/catalog-hierarchy/unf-native.tsv")
        .lines()
        .map(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            (fields[0], fields)
        })
        .collect::<HashMap<_, _>>();
    let pack = include_bytes!("fixtures/unf/config.pack");
    let mut offset = 0;
    let (mut hierarchical, mut nonhierarchical) = (0, 0);
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
                .find(|descriptor| descriptor.object_guid.as_str() == resource)
                .unwrap();
            assert_eq!(owner.name, fields[1]);
            let flag = match fields[2] {
                "true" => {
                    hierarchical += 1;
                    true
                }
                "false" => {
                    nonhierarchical += 1;
                    false
                }
                _ => panic!("unknown native flag"),
            };
            assert_eq!(owner.catalog_hierarchical, Some(flag), "{resource}");
            assert!(
                descriptors
                    .iter()
                    .filter(|descriptor| descriptor.object_guid.as_str() != resource)
                    .all(|descriptor| descriptor.catalog_hierarchical.is_none())
            );
        }
        offset += length;
    }
    assert_eq!((hierarchical, nonhierarchical), (77, 219));
}

#[test]
fn compact_native_prefixes_preserve_both_flags() {
    assert_eq!(owner(FALSE_GUID, FALSE).catalog_hierarchical, Some(false));
    assert_eq!(
        owner(
            include_str!("fixtures/catalog-hierarchy/true.guid"),
            include_str!("fixtures/catalog-hierarchy/true.serialized")
        )
        .catalog_hierarchical,
        Some(true)
    );
}

#[test]
fn unknown_or_missing_flags_do_not_default_to_false() {
    let prefix = FALSE.trim().strip_suffix("0}}").unwrap();
    for value in ["2", "-1", "\"0\"", ""] {
        let changed = format!("{prefix}{value}}}}}");
        assert!(
            owner(FALSE_GUID, &changed).catalog_hierarchical.is_none(),
            "{value}"
        );
    }
    for (before, after) in [("{57,", "{58,"), ("{1,{57,", "{2,{57,")] {
        assert!(FALSE.contains(before));
        assert!(
            owner(FALSE_GUID, &FALSE.replacen(before, after, 1))
                .catalog_hierarchical
                .is_none()
        );
    }
}
