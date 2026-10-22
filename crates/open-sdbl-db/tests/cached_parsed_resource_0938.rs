use open_sdbl::metadata::{DbNames, SchemaStorage, StorageLayout, parse_config_resource_bounded};
use open_sdbl_db::{
    ConfigResource, ConfigurationMetadataCache, ConfigurationMetadataInputs, Limits,
};

fn native_resource() -> ConfigResource {
    let bytes = include_bytes!("../../../tests/fixtures/demo/config.pack");
    let mut offset = 0;
    loop {
        let end = offset + bytes[offset..].iter().position(|b| *b == b'\n').unwrap();
        let (name, len) = std::str::from_utf8(&bytes[offset..end])
            .unwrap()
            .split_once('\t')
            .unwrap();
        let len: usize = len.parse().unwrap();
        if name.parse::<open_sdbl::metadata::Guid>().is_ok() {
            return ConfigResource {
                file_name: name.into(),
                compressed: bytes[end + 1..end + 1 + len].into(),
            };
        }
        offset = end + 1 + len;
    }
}

#[test]
fn native_facts_are_borrowed_unchanged_across_repeated_reads_and_resolution() {
    let resource = native_resource();
    let independent =
        parse_config_resource_bounded(&resource.file_name, &resource.compressed, 64 * 1024 * 1024)
            .unwrap();
    let mut cache = ConfigurationMetadataCache::new(Limits::default());
    assert!(cache.parsed_resource(&resource.file_name).is_none());
    cache.admit(&resource).unwrap();
    let first = cache.parsed_resource(&resource.file_name).unwrap();
    assert_eq!(first, &independent);
    let inputs = ConfigurationMetadataInputs {
        layout: StorageLayout::MODERN,
        db_names: DbNames::from_entries(vec![]),
        schema: SchemaStorage {
            tables: vec![],
            anomalies: vec![],
        },
        live_tables: vec![],
    };
    for _ in 0..3 {
        let _resolved = cache.resolve(&inputs, vec![]).unwrap();
        let next = cache.parsed_resource(&resource.file_name).unwrap();
        assert!(std::ptr::eq(first, next));
        assert_eq!(next, &independent);
    }
    assert!(cache.parsed_resource("absent").is_none());
    let upper = resource.file_name.to_ascii_uppercase();
    assert_ne!(upper, resource.file_name);
    assert!(cache.parsed_resource(&upper).is_none());
}

#[test]
fn rejected_admissions_leave_original_facts_available_and_new_names_absent() {
    let resource = native_resource();
    let independent =
        parse_config_resource_bounded(&resource.file_name, &resource.compressed, 64 * 1024 * 1024)
            .unwrap();
    let mut cache = ConfigurationMetadataCache::new(Limits {
        config_resource_limit: 2,
        config_retained_byte_limit: resource.compressed.len() * 2,
        ..Limits::default()
    });
    cache.admit(&resource).unwrap();
    let original = cache.parsed_resource(&resource.file_name).unwrap() as *const _;
    assert!(cache.admit(&resource).is_err());
    let broken = ConfigResource {
        file_name: "11111111-1111-4111-8111-111111111111".into(),
        compressed: vec![0xff].into(),
    };
    assert!(cache.admit(&broken).is_err());
    assert!(cache.parsed_resource(&broken.file_name).is_none());
    let oversized = ConfigResource {
        file_name: "22222222-2222-4222-8222-222222222222".into(),
        compressed: vec![0; resource.compressed.len() * 2 + 1].into(),
    };
    assert!(cache.admit(&oversized).is_err());
    assert!(cache.parsed_resource(&oversized.file_name).is_none());
    let retained = cache.parsed_resource(&resource.file_name).unwrap();
    assert_eq!(retained as *const _, original);
    assert_eq!(retained, &independent);
}
