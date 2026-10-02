use open_sdbl::metadata::{Guid, parse_config_descriptors};

#[test]
fn measured_buh_catalogs_acquire_complete_own_attributes() {
    let oracle = include_str!("fixtures/buh-catalog-layout-56/native.tsv");
    let pack = include_bytes!("fixtures/buh/config.pack");
    let mut offset = 0;
    let mut checked = 0;
    let mut attributes = 0;
    while offset < pack.len() {
        let end = offset + pack[offset..].iter().position(|b| *b == b'\n').unwrap();
        let (resource, size) = std::str::from_utf8(&pack[offset..end])
            .unwrap()
            .split_once('\t')
            .unwrap();
        let size: usize = size.parse().unwrap();
        offset = end + 1;
        if let Some(row) = oracle
            .lines()
            .find(|row| row.split('\t').next() == Some(resource))
        {
            let fields: Vec<_> = row.split('\t').collect();
            let descriptors =
                parse_config_descriptors(resource, &pack[offset..offset + size]).unwrap();
            let owner = descriptors
                .iter()
                .find(|d| d.object_guid.as_str() == resource)
                .unwrap();
            let ids: Vec<Guid> = fields[4]
                .split(',')
                .filter(|id| !id.is_empty())
                .map(|id| id.parse().unwrap())
                .collect();
            assert_eq!(owner.name, fields[1]);
            assert_eq!(owner.catalog_attributes.as_ref(), Some(&ids), "{resource}");
            for id in &ids {
                assert!(
                    descriptors.iter().any(|d| &d.object_guid == id),
                    "{resource}: {id}"
                );
            }
            attributes += ids.len();
            checked += 1;
        }
        offset += size;
    }
    assert_eq!((checked, attributes), (84, 733));
}

#[test]
fn same_name_common_module_does_not_acquire_catalog_membership() {
    let resource = "e5dfee58-f748-48ee-81a3-7c6133f06365";
    let descriptors = parse_config_descriptors(
        resource,
        include_bytes!("fixtures/buh-catalog-layout-56/same-name-common-module.deflate"),
    )
    .unwrap();
    let module = descriptors
        .iter()
        .find(|d| d.object_guid.as_str() == resource)
        .unwrap();
    assert_eq!(module.name, "ВнешниеПользователи");
    assert!(module.catalog_attributes.is_none());
}

#[test]
fn unknown_class_56_length_remains_unknown() {
    let resource = "92876c9e-52ff-480d-9878-5766f4f9d12c";
    let descriptors = parse_config_descriptors(
        resource,
        include_bytes!("fixtures/buh-catalog-layout-56/unknown-length.deflate"),
    )
    .unwrap();
    let owner = descriptors
        .iter()
        .find(|d| d.object_guid.as_str() == resource)
        .unwrap();
    assert!(owner.catalog_attributes.is_none());
    assert!(owner.catalog_owners.is_none());
}
