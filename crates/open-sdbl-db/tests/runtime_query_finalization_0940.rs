use open_sdbl::metadata::{
    LiveColumn, LiveTable, StorageLayout, parse_db_names, parse_schema_storage,
    resolve_selected_metadata, resolve_selected_runtime_metadata,
};
use open_sdbl_db::{
    ConfigResource, ConfigurationMetadataCache, ConfigurationMetadataInputs, Limits,
};

fn fixture(base: &str) -> (ConfigurationMetadataInputs, Vec<ConfigResource>) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(base);
    let mut live_tables: Vec<LiveTable> = Vec::new();
    if let Ok(text) = std::fs::read_to_string(root.join("live_columns.tsv")) {
        for line in text.lines().filter(|line| !line.is_empty()) {
            let mut parts = line.split('\t');
            let table = parts.next().unwrap();
            let column = parts.next().unwrap();
            let ty = parts.next().unwrap();
            let position = live_tables
                .iter()
                .position(|value| value.name == table)
                .unwrap_or_else(|| {
                    live_tables.push(LiveTable {
                        name: table.into(),
                        columns: vec![],
                        indexes: vec![],
                    });
                    live_tables.len() - 1
                });
            live_tables[position].columns.push(LiveColumn {
                name: column.into(),
                data_type: if ty == "USER-DEFINED" { "mvarchar" } else { ty }.into(),
            });
        }
    }
    let inputs = ConfigurationMetadataInputs {
        layout: StorageLayout::MODERN,
        db_names: parse_db_names(&std::fs::read(root.join("db_names.deflate")).unwrap()).unwrap(),
        schema: parse_schema_storage(&std::fs::read(root.join("schema_storage.txt")).unwrap())
            .unwrap(),
        live_tables,
    };
    let mut resources = Vec::new();
    if let Ok(bytes) = std::fs::read(root.join("config.pack")) {
        let mut offset = 0;
        while offset < bytes.len() {
            let end = offset
                + bytes[offset..]
                    .iter()
                    .position(|byte| *byte == b'\n')
                    .unwrap();
            let (name, count) = std::str::from_utf8(&bytes[offset..end])
                .unwrap()
                .split_once('\t')
                .unwrap();
            let count: usize = count.parse().unwrap();
            resources.push(ConfigResource {
                file_name: name.into(),
                compressed: bytes[end + 1..end + 1 + count].into(),
            });
            offset = end + 1 + count;
        }
    } else {
        for path in std::fs::read_dir(root.join("config"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
        {
            resources.push(ConfigResource {
                file_name: path
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .strip_suffix(".deflate")
                    .unwrap()
                    .into(),
                compressed: std::fs::read(path).unwrap().into(),
            });
        }
    }
    // Пакет содержит также исходники и неизвестные формы. Кэш принимает
    // только успешно разобранные ресурсы; отказы admission проверены отдельно.
    resources.retain(|resource| {
        open_sdbl::metadata::parse_config_resource_bounded(
            &resource.file_name,
            &resource.compressed,
            64 * 1024 * 1024,
        )
        .is_ok()
    });
    assert!(!resources.is_empty(), "{base}");
    resources.sort_by(|left, right| left.file_name.cmp(&right.file_name));
    (inputs, resources)
}

#[test]
fn finalization_preserves_complete_snapshots_and_facts_across_native_growth() {
    for base in ["demo", "buh", "unf", "separators"] {
        let (inputs, resources) = fixture(base);
        let mut cache = ConfigurationMetadataCache::new(Limits::default());
        let mut descriptors = Vec::new();
        let mut predefined = Vec::new();
        let mut older = Vec::new();
        for chunk in resources.chunks(resources.len().div_ceil(4).max(1)) {
            for resource in chunk {
                let parsed = open_sdbl::metadata::parse_config_resource_bounded(
                    &resource.file_name,
                    &resource.compressed,
                    64 * 1024 * 1024,
                )
                .unwrap();
                descriptors.extend(parsed.descriptors);
                predefined.extend(parsed.predefined_values);
                cache.admit(resource).unwrap();
            }
            let runtime = resolve_selected_runtime_metadata(
                &inputs.db_names,
                descriptors.clone(),
                predefined.clone(),
                vec![],
                &inputs.schema,
                &inputs.live_tables,
            );
            let expected = resolve_selected_metadata(
                &inputs.db_names,
                descriptors.clone(),
                predefined.clone(),
                vec![],
                &inputs.schema,
                &inputs.live_tables,
            );
            let actual = runtime.finalize_query();
            assert_eq!(
                runtime.descriptors().as_ptr(),
                actual.snapshot.descriptors().as_ptr()
            );
            assert_eq!(
                runtime.objects().as_ptr(),
                actual.snapshot.objects().as_ptr()
            );
            assert_eq!(runtime.values().as_ptr(), actual.snapshot.values().as_ptr());
            assert_eq!(
                actual.snapshot.fingerprint(),
                expected.snapshot.fingerprint(),
                "{base}"
            );
            assert_eq!(
                actual.snapshot.descriptors(),
                expected.snapshot.descriptors()
            );
            assert_eq!(actual.snapshot.objects(), expected.snapshot.objects());
            assert_eq!(actual.snapshot.fields(), expected.snapshot.fields());
            assert_eq!(actual.snapshot.values(), expected.snapshot.values());
            assert_eq!(
                format!("{:?}", actual.snapshot.indexes()),
                format!("{:?}", expected.snapshot.indexes())
            );
            assert_eq!(
                format!("{:?}", actual.report),
                format!("{:?}", expected.report)
            );
            for object in expected.snapshot.objects() {
                let id = open_sdbl::metadata::ObjectId::from(&object.guid);
                assert_eq!(
                    actual.snapshot.object_by_id(id),
                    expected.snapshot.object_by_id(id)
                );
                if let (Some(kind), Some(name)) = (object.kind, object.name.as_deref()) {
                    assert_eq!(
                        actual.snapshot.object_id(kind, name),
                        expected.snapshot.object_id(kind, name)
                    );
                }
                if let Some(table) = object.physical_table.as_deref() {
                    assert_eq!(
                        actual.snapshot.object_id_by_physical_table(table),
                        expected.snapshot.object_id_by_physical_table(table)
                    );
                }
            }
            let first = actual.snapshot.fingerprint();
            older.push((runtime.clone(), first));
            match (
                cache.resolve_runtime(&inputs, vec![]),
                cache.resolve(&inputs, vec![]),
            ) {
                (Ok(facts), Ok(full)) => {
                    let before = format!("{facts:?}");
                    let actual = cache.finalize_runtime(&facts).unwrap();
                    assert_eq!(actual.snapshot.fingerprint(), full.snapshot.fingerprint());
                    assert_eq!(actual.snapshot.criteria(), full.snapshot.criteria());
                    assert_eq!(actual.snapshot.roles(), full.snapshot.roles());
                    assert_eq!(format!("{facts:?}"), before);
                }
                (Err(a), Err(b)) => assert_eq!(format!("{a:?}"), format!("{b:?}")),
                (a, b) => panic!("{base}: {a:?}; {b:?}"),
            }
        }
        for (facts, fingerprint) in older {
            assert_eq!(facts.finalize_query().snapshot.fingerprint(), fingerprint);
        }
    }
}

#[test]
fn strict_finalization_diagnoses_missing_and_unsupported_separator_descriptors() {
    let (inputs, resources) = fixture("separators");
    let mut descriptors = Vec::new();
    let mut predefined = Vec::new();
    for resource in &resources {
        let parsed = open_sdbl::metadata::parse_config_resource_bounded(
            &resource.file_name,
            &resource.compressed,
            64 * 1024 * 1024,
        )
        .unwrap();
        descriptors.extend(parsed.descriptors);
        predefined.extend(parsed.predefined_values);
    }
    let complete = resolve_selected_runtime_metadata(
        &inputs.db_names,
        descriptors.clone(),
        predefined.clone(),
        vec![],
        &inputs.schema,
        &inputs.live_tables,
    );
    let guid = complete.separators().next().unwrap().guid.clone();
    descriptors.retain(|descriptor| descriptor.object_guid != guid);
    let missing = resolve_selected_runtime_metadata(
        &inputs.db_names,
        descriptors.clone(),
        predefined.clone(),
        vec![],
        &inputs.schema,
        &inputs.live_tables,
    );
    let mut cache = ConfigurationMetadataCache::new(Limits::default());
    let error = cache.finalize_runtime(&missing).unwrap_err();
    assert!(
        matches!(error, open_sdbl_db::DbError::ConfigurationDescriptorsMissing { ref objects } if objects.contains(&guid))
    );
    for resource in &resources {
        cache.admit(resource).unwrap();
    }
    let error = cache.finalize_runtime(&missing).unwrap_err();
    assert!(
        matches!(error, open_sdbl_db::DbError::Data(ref text) if text.contains("no supported own descriptor"))
    );
    let mut descriptors = complete.descriptors().to_vec();
    descriptors
        .iter_mut()
        .find(|descriptor| descriptor.object_guid == guid)
        .unwrap()
        .separation = None;
    let unsupported = resolve_selected_runtime_metadata(
        &inputs.db_names,
        descriptors,
        predefined,
        vec![],
        &inputs.schema,
        &inputs.live_tables,
    );
    let error = cache.finalize_runtime(&unsupported).unwrap_err();
    assert!(
        matches!(error, open_sdbl_db::DbError::Data(ref text) if text.contains("unsupported settings"))
    );
}

#[allow(clippy::duplicate_mod)]
#[path = "../../../tests/support/mod.rs"]
mod support;

#[test]
fn finalization_preserves_extension_normalization_and_deferred_diagnostics() {
    use open_sdbl::metadata::{
        ConfigDeclaredType, DbNames, ExtensionMetadata, SchemaStorage, resolve_selected_metadata,
        resolve_selected_runtime_metadata,
    };
    let root = support::guid("11111111-1111-4111-8111-111111111111");
    let field = support::guid("22222222-2222-4222-8222-222222222222");
    let defined = support::guid("55555555-5555-4555-8555-555555555555");
    let mut definition = support::descriptor(&root, &root, "Definition");
    definition.defined_type_id = Some(defined.clone());
    definition.declared_types = Some(vec![ConfigDeclaredType {
        tag: "B".into(),
        reference: None,
        qualifiers: vec![],
    }]);
    let mut attribute = support::descriptor(&root, &field, "Attribute");
    attribute.declared_types = Some(vec![ConfigDeclaredType {
        tag: "#".into(),
        reference: Some(defined),
        qualifiers: vec![],
    }]);
    let extension = ExtensionMetadata {
        origin: "measured extension".into(),
        db_names: DbNames::from_entries(vec![]),
        descriptors: vec![definition],
        schema: SchemaStorage {
            tables: vec![],
            anomalies: vec![],
        },
        field_reference_targets: vec![],
        restructure_anomalies: vec!["unsupported restructure shape".into()],
    };
    let names = DbNames::from_entries(vec![]);
    let schema = SchemaStorage {
        tables: vec![],
        anomalies: vec![],
    };
    let runtime = resolve_selected_runtime_metadata(
        &names,
        vec![attribute.clone()],
        vec![],
        vec![extension.clone()],
        &schema,
        &[],
    );
    let full = resolve_selected_metadata(
        &names,
        vec![attribute],
        vec![],
        vec![extension],
        &schema,
        &[],
    );
    let actual = runtime.finalize_query();
    assert_eq!(actual.snapshot.fingerprint(), full.snapshot.fingerprint());
    assert_eq!(actual.snapshot.descriptors(), full.snapshot.descriptors());
    assert_eq!(actual.snapshot.fields(), full.snapshot.fields());
    assert_eq!(format!("{:?}", actual.report), format!("{:?}", full.report));
    assert!(!actual.report.findings().is_empty());
    assert_eq!(
        runtime.descriptors()[0].declared_types.as_ref().unwrap()[0].tag,
        "B"
    );
}
