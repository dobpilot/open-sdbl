use open_sdbl::metadata::{Guid, parse_config_descriptors};

#[test]
fn every_native_enum_reference_identity_differs_from_its_manager() {
    let oracle = include_str!("fixtures/runtime-metadata-096/enum-native.tsv");
    let pack = include_bytes!("fixtures/unf/config.pack");
    let mut offset = 0;
    let mut checked = 0;
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
            let reference: Guid = fields[2].parse().unwrap();
            let manager: Guid = fields[3].parse().unwrap();
            assert_ne!(reference, manager);
            assert_eq!(owner.name, fields[1]);
            assert_eq!(owner.object_reference_type, Some(reference), "{resource}");
            assert_ne!(owner.object_reference_type, Some(manager));
            checked += 1;
        }
        offset += size;
    }
    assert_eq!(checked, 426);
}
