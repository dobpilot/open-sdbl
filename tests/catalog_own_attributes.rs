mod support;

use open_sdbl::metadata::{ConfigDescriptor, Guid, parse_config_descriptors};
use std::collections::HashMap;
use support::stored_deflate;

const RESOURCE: &str = "92876c9e-52ff-480d-9878-5766f4f9d12c";
const COLLECTION: &str = "cf4abea7-37b2-11d4-940f-008048da11f9";
const COMPACT: &str = include_str!("fixtures/catalog-own-attributes/compact.serialized");

fn compact_owner(text: &str) -> ConfigDescriptor {
    parse_config_descriptors(RESOURCE, &stored_deflate(text.as_bytes()))
        .unwrap()
        .into_iter()
        .find(|descriptor| descriptor.object_guid.to_string() == RESOURCE)
        .unwrap()
}

#[test]
fn own_membership_matches_native_order_and_excludes_all_tabular_attributes() {
    let expected = include_str!("fixtures/catalog-own-attributes/unf-native.tsv")
        .lines()
        .map(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            (fields[0], fields)
        })
        .collect::<HashMap<_, _>>();
    let pack = include_bytes!("fixtures/unf/config.pack");
    let mut offset = 0;
    let (mut catalogs, mut own_count, mut tabular_count, mut empty_count) = (0, 0, 0, 0);
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
            let own = fields[2]
                .split(',')
                .filter(|value| !value.is_empty())
                .map(|value| value.split_once('=').unwrap())
                .collect::<Vec<_>>();
            let ids = own
                .iter()
                .map(|(id, _)| id.parse::<Guid>().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(owner.catalog_attributes.as_ref(), Some(&ids), "{resource}");
            for (id, name) in &own {
                let descriptor = descriptors
                    .iter()
                    .find(|descriptor| descriptor.object_guid.as_str() == *id)
                    .unwrap();
                assert_eq!(&descriptor.name, name);
            }
            for id in fields[3].split(',').filter(|value| !value.is_empty()) {
                assert!(!ids.contains(&id.parse::<Guid>().unwrap()));
                tabular_count += 1;
            }
            assert!(
                descriptors
                    .iter()
                    .filter(|descriptor| descriptor.object_guid.to_string() != resource)
                    .all(|descriptor| descriptor.catalog_attributes.is_none())
            );
            if resource == RESOURCE {
                assert_eq!(ids.len(), 8);
                assert!(ids.contains(&"d65c4485-7bac-4206-9a6c-7156f4bb876d".parse().unwrap()));
                assert_eq!(fields[3].split(',').count(), 3);
            }
            catalogs += 1;
            own_count += own.len();
            empty_count += usize::from(own.is_empty());
        }
        offset += length;
    }
    assert_eq!(
        (catalogs, own_count, tabular_count, empty_count),
        (296, 3540, 1453, 19)
    );
}

#[test]
fn compact_and_known_empty_collections_preserve_membership() {
    let owner = compact_owner(COMPACT);
    assert_eq!(
        owner.catalog_attributes.unwrap(),
        vec![
            "b729d739-5249-4765-85ee-fd792bb22911"
                .parse::<Guid>()
                .unwrap(),
            "d65c4485-7bac-4206-9a6c-7156f4bb876d"
                .parse::<Guid>()
                .unwrap(),
        ]
    );
    let empty = compact_owner(include_str!(
        "fixtures/catalog-own-attributes/empty.serialized"
    ));
    assert_eq!(empty.catalog_attributes, Some(Vec::new()));
}

#[test]
fn incomplete_or_ambiguous_collections_never_return_partial_membership() {
    for (before, after) in [
        (
            "cf4abea7-37b2-11d4-940f-008048da11f9,2",
            "cf4abea7-37b2-11d4-940f-008048da11f9,3",
        ),
        (
            "cf4abea7-37b2-11d4-940f-008048da11f9,2",
            "cf4abea7-37b2-11d4-940f-008048da11f9,1",
        ),
        (
            "cf4abea7-37b2-11d4-940f-008048da11f9,2",
            "cf4abea7-37b2-11d4-940f-008048da11f9,\"2\"",
        ),
        (COLLECTION, "cf4abea9-37b2-11d4-940f-008048da11f9"),
        (
            "d65c4485-7bac-4206-9a6c-7156f4bb876d",
            "b729d739-5249-4765-85ee-fd792bb22911",
        ),
        (
            "d65c4485-7bac-4206-9a6c-7156f4bb876d",
            "00000000-0000-0000-0000-000000000000",
        ),
        ("\"ОбъектАвторизации\"", "0"),
    ] {
        assert!(COMPACT.contains(before));
        let changed = COMPACT.replacen(before, after, 1);
        assert!(
            compact_owner(&changed).catalog_attributes.is_none(),
            "{after}"
        );
    }
    let prefix = COMPACT.trim().strip_suffix('}').unwrap();
    let duplicate = format!("{prefix},{{{COLLECTION},0}}}}");
    assert!(compact_owner(&duplicate).catalog_attributes.is_none());
    let scalar = COMPACT.replace(&format!("{COLLECTION},2,"), &format!("{COLLECTION},2,0,"));
    assert!(compact_owner(&scalar).catalog_attributes.is_none());
}
