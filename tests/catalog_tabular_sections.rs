mod support;

use open_sdbl::metadata::{ConfigDescriptor, Guid, parse_config_descriptors};
use std::collections::HashMap;
use support::stored_deflate;

const RESOURCE: &str = "a2782760-dd4c-44d2-a785-814d6f610455";
const COLLECTION: &str = "932159f9-95b2-4e76-a8dd-8849fe5c5ded";
const COMPACT: &str = include_str!("fixtures/catalog-tabular-sections/compact.serialized");

fn compact_owner(text: &str) -> ConfigDescriptor {
    parse_config_descriptors(RESOURCE, &stored_deflate(text.as_bytes()))
        .unwrap()
        .into_iter()
        .find(|descriptor| descriptor.object_guid.to_string() == RESOURCE)
        .unwrap()
}

#[test]
fn tabular_membership_matches_all_native_catalogs() {
    let expected = include_str!("fixtures/catalog-tabular-sections/unf-native.tsv")
        .lines()
        .map(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            (fields[0], fields)
        })
        .collect::<HashMap<_, _>>();
    let pack = include_bytes!("fixtures/unf/config.pack");
    let mut offset = 0;
    let (mut catalogs, mut own_count, mut empty_count) = (0, 0, 0);
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
            assert_eq!(
                owner.catalog_tabular_sections.as_ref(),
                Some(&ids),
                "{resource}"
            );
            for (id, name) in &own {
                let descriptor = descriptors
                    .iter()
                    .find(|descriptor| descriptor.object_guid.as_str() == *id)
                    .unwrap();
                assert_eq!(&descriptor.name, name);
            }
            assert!(
                descriptors
                    .iter()
                    .filter(|descriptor| descriptor.object_guid.to_string() != resource)
                    .all(|descriptor| descriptor.catalog_tabular_sections.is_none())
            );
            if resource == RESOURCE {
                assert_eq!(
                    ids,
                    vec![
                        "6581ac1f-11ef-42b8-9f3b-590bb7546e63"
                            .parse::<Guid>()
                            .unwrap()
                    ]
                );
            }
            catalogs += 1;
            own_count += own.len();
            empty_count += usize::from(own.is_empty());
        }
        offset += length;
    }
    assert_eq!((catalogs, own_count, empty_count), (296, 313, 151));
}

#[test]
fn compact_membership_and_known_empty_are_preserved() {
    assert_eq!(
        compact_owner(COMPACT)
            .catalog_tabular_sections
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        compact_owner(include_str!(
            "fixtures/catalog-tabular-sections/empty.serialized"
        ))
        .catalog_tabular_sections,
        Some(Vec::new())
    );
}

#[test]
fn incomplete_or_unknown_membership_is_not_guessed() {
    for changed in [
        COMPACT.replacen(
            &format!("{{{COLLECTION},1,"),
            &format!("{{{COLLECTION},2,"),
            1,
        ),
        COMPACT.replace(
            "6581ac1f-11ef-42b8-9f3b-590bb7546e63",
            "00000000-0000-0000-0000-000000000000",
        ),
        COMPACT.replace(COLLECTION, "00000000-0000-0000-0000-000000000000"),
        COMPACT.replace("\"Состав\"", "0"),
        COMPACT.replace(
            &format!("{{{COLLECTION},1,"),
            &format!("{{{COLLECTION},\"1\","),
        ),
        format!(
            "{},{{{COLLECTION},0}}}}",
            COMPACT.strip_suffix('}').unwrap()
        ),
    ] {
        assert!(compact_owner(&changed).catalog_tabular_sections.is_none());
    }
}
