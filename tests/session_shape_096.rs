use open_sdbl::metadata::{
    CatalogHierarchyType, CommonAttributeContentUse, CommonAttributeUse, ConfigDescriptor, DbNames,
    Guid, MetadataSnapshot, SchemaStorage, Value, parse_config_descriptors, resolve_metadata,
};

fn snapshot(descriptors: Vec<ConfigDescriptor>) -> MetadataSnapshot {
    resolve_metadata(
        DbNames::from_entries(Vec::new()),
        descriptors,
        SchemaStorage {
            tables: Vec::new(),
            anomalies: Vec::new(),
        },
        Vec::new(),
    )
    .snapshot
}

fn owner(resource: &str, bytes: &[u8]) -> ConfigDescriptor {
    let snapshot = snapshot(parse_config_descriptors(resource, bytes).unwrap());
    snapshot
        .descriptors()
        .iter()
        .find(|d| d.object_guid.as_str() == resource)
        .unwrap()
        .clone()
}

#[test]
fn native_catalog_modes_and_complete_owners_survive_snapshot_acquisition() {
    let oracle = include_str!("fixtures/session-shape-096/catalog-native.tsv");
    let mut checked = 0;
    let mut owner_counts = [0; 4];
    let mut modes = [0; 2];
    for (db, pack) in [
        (
            "demo",
            include_bytes!("fixtures/demo/config.pack").as_slice(),
        ),
        ("buh", include_bytes!("fixtures/buh/config.pack").as_slice()),
        ("unf", include_bytes!("fixtures/unf/config.pack").as_slice()),
    ] {
        let mut offset = 0;
        while offset < pack.len() {
            let end = offset + pack[offset..].iter().position(|b| *b == b'\n').unwrap();
            let (resource, size) = std::str::from_utf8(&pack[offset..end])
                .unwrap()
                .split_once('\t')
                .unwrap();
            let size: usize = size.parse().unwrap();
            offset = end + 1;
            if let Some(row) = oracle.lines().find(|row| {
                let mut fields = row.split('\t');
                fields.next() == Some(db) && fields.next() == Some(resource)
            }) {
                let fields: Vec<_> = row.split('\t').collect();
                let descriptor = owner(resource, &pack[offset..offset + size]);
                assert_eq!(descriptor.name, fields[2]);
                let mode = match fields[3] {
                    "HierarchyFoldersAndItems" => CatalogHierarchyType::FoldersAndItems,
                    "HierarchyOfItems" => CatalogHierarchyType::Items,
                    _ => panic!(),
                };
                assert_eq!(
                    descriptor.catalog_hierarchy_type,
                    Some(mode),
                    "{db}:{resource}"
                );
                let owners: Vec<Guid> = fields[5]
                    .split(',')
                    .filter(|id| !id.is_empty())
                    .map(|id| id.parse().unwrap())
                    .collect();
                assert_eq!(
                    descriptor.catalog_owners.as_ref(),
                    Some(&owners),
                    "{db}:{resource}"
                );
                owner_counts[owners.len()] += 1;
                modes[usize::from(mode == CatalogHierarchyType::Items)] += 1;
                checked += 1;
            }
            offset += size;
        }
    }
    assert_eq!(checked, 737);
    assert_eq!(owner_counts, [674, 55, 5, 3]);
    assert_eq!(modes, [659, 78]);
}

#[test]
fn native_common_attributes_preserve_complete_explicit_content_and_auto_use() {
    let oracle = include_str!("fixtures/session-shape-096/common-native.tsv");
    let mut checked = 0;
    let mut entries = 0;
    for row in oracle.lines() {
        let fields: Vec<_> = row.split('\t').collect();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/session-shape-096")
            .join(format!("{}-{}.deflate", fields[0], fields[2]));
        let descriptor = owner(fields[1], &std::fs::read(path).unwrap());
        let usage = descriptor
            .common_attribute_usage
            .as_ref()
            .unwrap_or_else(|| panic!("{}:{}", fields[0], fields[2]));
        let parse_use = |value| match value {
            "Use" => CommonAttributeUse::Use,
            "DontUse" => CommonAttributeUse::DontUse,
            _ => panic!(),
        };
        assert_eq!(descriptor.name, fields[2]);
        assert_eq!(usage.auto_use, parse_use(fields[3]));
        let expected: Vec<_> = fields[4]
            .split(',')
            .filter(|value| !value.is_empty())
            .map(|value| {
                let (id, usage) = value.split_once(':').unwrap();
                (
                    id.parse::<Guid>().unwrap(),
                    match parse_use(usage) {
                        CommonAttributeUse::Use => CommonAttributeContentUse::Use,
                        CommonAttributeUse::DontUse => CommonAttributeContentUse::DontUse,
                    },
                )
            })
            .collect();
        let actual: Vec<_> = usage
            .content
            .iter()
            .map(|entry| (entry.object.clone(), entry.usage))
            .collect();
        assert_eq!(actual, expected, "{}:{}", fields[0], fields[1]);
        assert!(descriptor.declared_types.is_some());
        assert!(descriptor.catalog_attributes.is_none());
        entries += actual.len();
        checked += 1;
    }
    assert_eq!((checked, entries), (16, 1584));
}

#[test]
fn demo_common_name_and_string_qualifiers_are_acquired_for_its_explicit_catalog() {
    let descriptor = owner(
        "03f9d2d0-eae4-4098-a70e-4eb569b11fd9",
        include_bytes!("fixtures/session-shape-096/demo-НаименованиеЯзык1.deflate"),
    );
    assert_eq!(descriptor.name, "НаименованиеЯзык1");
    let usage = descriptor.common_attribute_usage.unwrap();
    assert_eq!(usage.auto_use, CommonAttributeUse::DontUse);
    assert_eq!(usage.content.len(), 51);
    assert_eq!(
        usage
            .content
            .iter()
            .find(|entry| entry.object.as_str() == "9e9e2a57-8536-4983-a5c0-7841ead585e8")
            .unwrap()
            .usage,
        CommonAttributeContentUse::Use
    );
    assert!(
        !usage
            .content
            .iter()
            .any(|entry| entry.object.as_str() == "0169b236-52cf-4e6a-966e-2e180783b83b")
    );
    let types = descriptor.declared_types.unwrap();
    assert_eq!(types.len(), 1);
    assert_eq!(types[0].tag, "S");
    assert_eq!(
        types[0].qualifiers,
        vec![Value::Atom("150".into()), Value::Atom("1".into())]
    );
}

#[test]
fn unsupported_or_incomplete_common_usage_is_never_partial() {
    let id = "03f9d2d0-eae4-4098-a70e-4eb569b11fd9";
    for bytes in [
        include_bytes!("fixtures/session-shape-096/common-bad-count.deflate").as_slice(),
        include_bytes!("fixtures/session-shape-096/common-unknown-use.deflate").as_slice(),
        include_bytes!("fixtures/session-shape-096/common-unknown-auto.deflate").as_slice(),
        include_bytes!("fixtures/session-shape-096/common-unknown-class.deflate").as_slice(),
    ] {
        assert!(owner(id, bytes).common_attribute_usage.is_none());
    }
    let empty = owner(
        id,
        include_bytes!("fixtures/session-shape-096/common-empty.deflate"),
    );
    assert!(empty.common_attribute_usage.unwrap().content.is_empty());
}

#[test]
fn catalog_unknown_codes_and_incomplete_owners_remain_unknown() {
    let id = "0169b236-52cf-4e6a-966e-2e180783b83b";
    assert!(
        owner(
            id,
            include_bytes!("fixtures/session-shape-096/catalog-bad-owner-count.deflate")
        )
        .catalog_owners
        .is_none()
    );
    assert!(
        owner(
            id,
            include_bytes!("fixtures/session-shape-096/catalog-unknown-hierarchy.deflate")
        )
        .catalog_hierarchy_type
        .is_none()
    );
    let unknown = owner(
        id,
        include_bytes!("fixtures/session-shape-096/catalog-unknown-layout.deflate"),
    );
    assert!(unknown.catalog_hierarchy_type.is_none());
    assert!(unknown.catalog_owners.is_none());
    let known = owner(
        id,
        include_bytes!("fixtures/session-shape-096/buh-РегистрацииВНалоговомОргане.deflate"),
    );
    assert_eq!(
        known.catalog_owners,
        Some(vec![
            "fd0c3124-91f5-4c1e-bbc0-f2163e61ff2a".parse().unwrap()
        ])
    );
}
