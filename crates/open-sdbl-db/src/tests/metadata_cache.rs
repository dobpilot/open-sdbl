use super::*;
use crate::resolve_configuration_resources;
use open_sdbl::metadata::{DbNames, SchemaStorage, StorageLayout};
use std::sync::Arc;

fn empty_inputs() -> ConfigurationMetadataInputs {
    ConfigurationMetadataInputs {
        layout: StorageLayout::MODERN,
        db_names: DbNames::from_entries(vec![]),
        schema: SchemaStorage {
            tables: vec![],
            anomalies: vec![],
        },
        live_tables: vec![],
    }
}
fn pack(base: &str) -> Vec<ConfigResource> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(base)
        .join("config.pack");
    let bytes = std::fs::read(path).unwrap();
    let mut offset = 0;
    let mut result = Vec::new();
    while offset < bytes.len() {
        let end = offset + bytes[offset..].iter().position(|b| *b == b'\n').unwrap();
        let (name, len) = std::str::from_utf8(&bytes[offset..end])
            .unwrap()
            .split_once('\t')
            .unwrap();
        let len: usize = len.parse().unwrap();
        result.push(ConfigResource {
            file_name: name.into(),
            compressed: bytes[end + 1..end + 1 + len].into(),
        });
        offset = end + 1 + len;
    }
    result
}
fn parity(
    cache: &ConfigurationMetadataCache,
    resources: &[ConfigResource],
    inputs: &ConfigurationMetadataInputs,
    extensions: Vec<ExtensionMetadata>,
) {
    let cached = cache.resolve(inputs, extensions.clone()).unwrap();
    let plain = resolve_configuration_resources(inputs, resources, extensions).unwrap();
    assert_eq!(cached.report, plain.report);
    assert_eq!(cached.snapshot.descriptors(), plain.snapshot.descriptors());
    assert_eq!(cached.snapshot.objects(), plain.snapshot.objects());
    assert_eq!(cached.snapshot.fields(), plain.snapshot.fields());
    assert_eq!(cached.snapshot.values(), plain.snapshot.values());
    assert_eq!(cached.snapshot.roles(), plain.snapshot.roles());
    assert_eq!(cached.snapshot.criteria(), plain.snapshot.criteria());
    assert_eq!(cached.snapshot.fingerprint(), plain.snapshot.fingerprint());
}
#[test]
fn native_admission_drops_compressed_buffers_and_never_needs_old_bytes_again() {
    let inputs = empty_inputs();
    for base in ["demo", "buh", "unf"] {
        let mut resources = pack(base)
            .into_iter()
            .filter(|r| r.file_name.parse::<Guid>().is_ok())
            .take(5)
            .collect::<Vec<_>>();
        let mut cache = ConfigurationMetadataCache::new(Limits::default());
        for resource in &resources {
            let before = Arc::strong_count(&resource.compressed);
            cache.admit(resource).unwrap();
            assert_eq!(Arc::strong_count(&resource.compressed), before);
        }
        parity(&cache, &resources, &inputs, vec![]);
        let weak = Arc::downgrade(&resources[0].compressed);
        resources[0].compressed = vec![0xff].into();
        assert!(weak.upgrade().is_none());
        let before = cache
            .resolve(&inputs, vec![])
            .unwrap()
            .snapshot
            .fingerprint();
        assert!(matches!(cache.admit(&resources[0]), Err(DbError::Data(_))));
        drop(resources);
        for _ in 0..3 {
            assert_eq!(
                cache
                    .resolve(&inputs, vec![])
                    .unwrap()
                    .snapshot
                    .fingerprint(),
                before
            );
        }
    }
}
#[test]
fn admission_failures_preserve_facts_and_budgets() {
    let resource = pack("demo")
        .into_iter()
        .find(|r| r.file_name.parse::<Guid>().is_ok())
        .unwrap();
    let limits = Limits {
        config_resource_limit: 1,
        config_retained_byte_limit: resource.compressed.len(),
        ..Limits::default()
    };
    let mut cache = ConfigurationMetadataCache::new(limits);
    let mut broken = resource.clone();
    broken.compressed = vec![0xff].into();
    for _ in 0..2 {
        assert!(cache.admit(&broken).is_err());
        assert!(cache.facts.is_empty());
        assert_eq!(cache.decoded_bytes, 0);
        assert_eq!(cache.compressed_bytes, 0);
    }
    cache.admit(&resource).unwrap();
    let mut next = resource.clone();
    next.file_name = "another".into();
    let before = cache
        .resolve(&empty_inputs(), vec![])
        .unwrap()
        .snapshot
        .fingerprint();
    assert!(cache.admit(&next).is_err());
    assert_eq!(
        cache
            .resolve(&empty_inputs(), vec![])
            .unwrap()
            .snapshot
            .fingerprint(),
        before
    );
}

#[allow(clippy::duplicate_mod)]
#[path = "../../../../tests/support/mod.rs"]
mod support;

#[test]
fn native_separator_admission_preserves_strict_failures_and_complete_projection() {
    use open_sdbl::metadata::{parse_db_names, parse_schema_storage};
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/separators");
    let inputs = ConfigurationMetadataInputs {
        layout: StorageLayout::MODERN,
        db_names: parse_db_names(&std::fs::read(root.join("db_names.deflate")).unwrap()).unwrap(),
        schema: parse_schema_storage(&std::fs::read(root.join("schema_storage.txt")).unwrap())
            .unwrap(),
        live_tables: vec![],
    };
    let mut resources = std::fs::read_dir(root.join("config"))
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            ConfigResource {
                file_name: path
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .strip_suffix(".deflate")
                    .unwrap()
                    .into(),
                compressed: std::fs::read(path).unwrap().into(),
            }
        })
        .collect::<Vec<_>>();
    resources.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    let mut cache = ConfigurationMetadataCache::new(Limits::default());
    let mut selected = Vec::new();
    for resource in resources {
        cache.admit(&resource).unwrap();
        selected.push(resource);
        match (
            cache.resolve(&inputs, vec![]),
            resolve_configuration_resources(&inputs, &selected, vec![]),
        ) {
            (Ok(_), Ok(_)) => parity(&cache, &selected, &inputs, vec![]),
            (
                Err(DbError::ConfigurationDescriptorsMissing { objects: cached }),
                Err(DbError::ConfigurationDescriptorsMissing { objects: plain }),
            ) => assert_eq!(cached, plain),
            (cached, plain) => panic!("cached={cached:?}; plain={plain:?}"),
        }
    }
    let complete = cache.resolve(&inputs, vec![]).unwrap();
    assert!(complete.snapshot.separators().next().is_some());
    parity(&cache, &selected, &inputs, vec![]);
}

#[test]
fn explicit_extension_and_prepared_queries_use_the_cached_snapshot_fingerprint() {
    use open_sdbl::metadata::DbNameEntry;
    use open_sdbl::query::{PostgresBackend, QueryCompiler};
    let full = support::snapshot();
    let resource = ConfigResource {
        file_name: "b8bac76b-c91b-4d78-8a70-ffa39f8de694".into(),
        compressed: support::hex(
            "4d8d4b0ac3201400af22ae7d9018a3be650f505ae809def303857e426256c1bb37d850ba9e6166d36adb7ad529f64cc15986803d8389ce8327d741ce3460f631593455c9cb945eb7c88f732a14a9d0757e73926a4fc879955f2e965d10cfc31053536a3d467a0c68390e902918303a65608b23381390b219468fbc8f5af854ca7ce7b5fc1f1a10f423b5d60f",
        ).into(),
    };
    let inputs = ConfigurationMetadataInputs {
        layout: StorageLayout::MODERN,
        db_names: full.db_names().clone(),
        schema: full.schema().clone(),
        live_tables: full.live_tables().to_vec(),
    };
    let mut cache = ConfigurationMetadataCache::new(Limits::default());
    cache.admit(&resource).unwrap();
    let first = cache.resolve(&inputs, vec![]).unwrap().snapshot;
    let owner = &first.objects()[0].guid;
    let field = support::guid("33333333-3333-4333-8333-333333333333");
    let mut schema = inputs.schema.clone();
    let mut column = schema.tables[0].columns.last().unwrap().clone();
    column.name = "Fld999".into();
    schema.tables[0].columns.push(column);
    let extension = ExtensionMetadata {
        origin: "explicit".into(),
        db_names: DbNames::from_entries(vec![DbNameEntry {
            guid: field.clone(),
            alias: "Fld".into(),
            number: 999,
        }]),
        descriptors: vec![support::descriptor(owner, &field, "ExtensionField")],
        schema,
        field_reference_targets: vec![],
        restructure_anomalies: vec![],
    };
    parity(&cache, &[resource], &inputs, vec![extension.clone()]);
    let second = cache.resolve(&inputs, vec![extension]).unwrap().snapshot;
    let projected = second
        .fields()
        .iter()
        .find(|value| value.guid == field)
        .unwrap();
    assert_eq!(projected.extension_origin.as_deref(), Some("explicit"));
    assert!(
        projected
            .owner_tables
            .iter()
            .any(|table| table == "_Reference53")
    );
    let prepared = QueryCompiler::new(&first, PostgresBackend)
        .prepare("SELECT Code FROM Catalog.OpenSdblMetadataProbe")
        .unwrap();
    assert!(prepared.compile(&first, &[]).is_ok());
    assert!(prepared.compile(&second, &[]).is_err());
    assert_eq!(
        first.fingerprint(),
        cache
            .resolve(&inputs, vec![])
            .unwrap()
            .snapshot
            .fingerprint()
    );
}

#[test]
fn compressed_budget_failure_does_not_poison_a_later_valid_admission() {
    let resource = pack("demo")
        .into_iter()
        .find(|resource| resource.file_name.parse::<Guid>().is_ok())
        .unwrap();
    let mut cache = ConfigurationMetadataCache::new(Limits {
        config_resource_limit: 10,
        config_retained_byte_limit: resource.compressed.len(),
        ..Limits::default()
    });
    let mut oversized = resource.clone();
    oversized.compressed = vec![0; resource.compressed.len() + 1].into();
    for _ in 0..2 {
        assert!(cache.admit(&oversized).is_err());
        assert!(cache.names.is_empty());
        assert_eq!(cache.compressed_bytes, 0);
    }
    cache.admit(&resource).unwrap();
    let before = cache
        .resolve(&empty_inputs(), vec![])
        .unwrap()
        .snapshot
        .fingerprint();
    oversized.file_name = "later".into();
    assert!(cache.admit(&oversized).is_err());
    assert_eq!(cache.names.len(), 1);
    assert_eq!(
        cache
            .resolve(&empty_inputs(), vec![])
            .unwrap()
            .snapshot
            .fingerprint(),
        before
    );
}
