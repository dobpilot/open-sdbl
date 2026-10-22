use super::*;
use open_sdbl::metadata::{
    DbNameEntry, DbNames, LiveColumn, LiveTable, MetadataSnapshot, RuntimeMetadataFacts,
    SchemaStorage, StorageLayout, parse_db_names, parse_schema_storage,
};

#[allow(clippy::duplicate_mod)]
#[path = "../../../../tests/support/mod.rs"]
mod support;

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

fn agree(facts: &RuntimeMetadataFacts, snapshot: &MetadataSnapshot) {
    assert_eq!(facts.descriptors(), snapshot.descriptors());
    assert_eq!(facts.objects(), snapshot.objects());
    assert_eq!(facts.values(), snapshot.values());
    assert_eq!(
        format!("{:?}", facts.schema()),
        format!("{:?}", snapshot.schema())
    );
    assert_eq!(
        facts.separators().collect::<Vec<_>>(),
        snapshot.separators().collect::<Vec<_>>()
    );
    assert_eq!(facts.criteria(), snapshot.criteria());
    assert_eq!(facts.roles(), snapshot.roles());
    for object in snapshot.objects() {
        let id = open_sdbl::metadata::ObjectId::from(&object.guid);
        assert_eq!(facts.object_by_id(id), snapshot.object_by_id(id));
    }
    for descriptor in snapshot.descriptors() {
        let mut matching = snapshot
            .fields()
            .iter()
            .filter(|field| field.guid == descriptor.object_guid);
        let expected = matching
            .next()
            .map(|field| field.data_separator)
            .filter(|_| matching.next().is_none());
        assert_eq!(
            facts.common_attribute_separation(&descriptor.object_guid),
            expected
        );
    }
}

#[test]
fn growing_native_runtime_facts_match_complete_snapshots_and_keep_old_facts_immutable() {
    for base in ["demo", "buh", "unf", "separators"] {
        let (inputs, resources) = fixture(base);
        let mut cache = ConfigurationMetadataCache::new(Limits::default());
        let mut previous = Vec::new();
        let step = resources.len().div_ceil(4).max(1);
        for chunk in resources.chunks(step) {
            for resource in chunk {
                cache.admit(resource).unwrap();
            }
            // Корпуса demo/buh/unf обрезаны и могут не содержать обязательные
            // описатели разделения; сравниваем и общий core, и строгий SDK.
            let mut descriptors = Vec::new();
            let mut predefined = Vec::new();
            for parsed in &cache.facts {
                descriptors.extend(parsed.descriptors.clone());
                predefined.extend(parsed.predefined_values.clone());
            }
            let runtime = open_sdbl::metadata::resolve_selected_runtime_metadata(
                &inputs.db_names,
                descriptors.clone(),
                predefined.clone(),
                vec![],
                &inputs.schema,
                &inputs.live_tables,
            );
            let full = open_sdbl::metadata::resolve_selected_metadata(
                &inputs.db_names,
                descriptors,
                predefined,
                vec![],
                &inputs.schema,
                &inputs.live_tables,
            );
            agree(&runtime, &full.snapshot);
            let before = format!("{runtime:?}");
            previous.push((runtime, before));
            match (
                cache.resolve_runtime(&inputs, vec![]),
                cache.resolve(&inputs, vec![]),
            ) {
                (Ok(runtime), Ok(full)) => {
                    agree(&runtime, &full.snapshot);
                    agree(full.snapshot.runtime_facts(), &full.snapshot);
                    let before = format!("{runtime:?}");
                    previous.push((runtime, before));
                }
                (Err(runtime), Err(full)) => {
                    assert_eq!(format!("{runtime:?}"), format!("{full:?}"))
                }
                (runtime, full) => panic!("{base}: runtime={runtime:?}; full={full:?}"),
            }
        }
        assert!(!previous.is_empty(), "{base}");
        for (facts, original) in previous {
            assert_eq!(format!("{facts:?}"), original);
        }
    }
}

#[test]
fn runtime_classification_distinguishes_absent_ordinary_separator_and_ambiguous_fields() {
    let guid = support::guid("33333333-3333-4333-8333-333333333333");
    let mut inputs = ConfigurationMetadataInputs {
        layout: StorageLayout::MODERN,
        db_names: DbNames::from_entries(vec![DbNameEntry {
            guid: guid.clone(),
            alias: "Fld".into(),
            number: 7,
        }]),
        schema: SchemaStorage {
            tables: vec![],
            anomalies: vec![],
        },
        live_tables: vec![],
    };
    let descriptor = support::descriptor(&guid, &guid, "Attribute");
    let ordinary = open_sdbl::metadata::resolve_selected_runtime_metadata(
        &inputs.db_names,
        vec![descriptor.clone()],
        vec![],
        vec![],
        &inputs.schema,
        &[],
    );
    assert_eq!(ordinary.common_attribute_separation(&guid), Some(false));
    assert_eq!(
        ordinary
            .common_attribute_separation(&support::guid("44444444-4444-4444-8444-444444444444")),
        None
    );
    // Классификацию разделителя доказывает авторитетный DBNames базы,
    // а не синтетический from_entries без измеренных separator-флагов.
    let (separator_inputs, _) = fixture("separators");
    let separator_guid = separator_inputs
        .db_names
        .entries()
        .iter()
        .find(|entry| {
            entry.alias == "Fld" && separator_inputs.db_names.is_data_separator(entry.number)
        })
        .unwrap()
        .guid
        .clone();
    let separator = open_sdbl::metadata::resolve_selected_runtime_metadata(
        &separator_inputs.db_names,
        vec![support::descriptor(
            &separator_guid,
            &separator_guid,
            "Separator",
        )],
        vec![],
        vec![],
        &separator_inputs.schema,
        &[],
    );
    assert_eq!(
        separator.common_attribute_separation(&separator_guid),
        Some(true)
    );
    inputs.db_names = DbNames::from_entries(vec![
        DbNameEntry {
            guid: guid.clone(),
            alias: "Fld".into(),
            number: 7,
        },
        DbNameEntry {
            guid: guid.clone(),
            alias: "Fld".into(),
            number: 8,
        },
    ]);
    let ambiguous = open_sdbl::metadata::resolve_selected_runtime_metadata(
        &inputs.db_names,
        vec![descriptor],
        vec![],
        vec![],
        &inputs.schema,
        &[],
    );
    assert_eq!(ambiguous.common_attribute_separation(&guid), None);
}

#[test]
fn runtime_normalization_and_explicit_extensions_match_the_full_resolver() {
    use open_sdbl::metadata::{
        ConfigDeclaredType, resolve_selected_metadata, resolve_selected_runtime_metadata,
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
        restructure_anomalies: vec![],
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
    agree(&runtime, &full.snapshot);
    assert_eq!(
        runtime.descriptors()[0].declared_types.as_ref().unwrap()[0].tag,
        "B"
    );
}

#[test]
fn runtime_strict_errors_match_unsupported_and_admitted_missing_separator_facts() {
    let (inputs, resources) = fixture("separators");
    let mut cache = ConfigurationMetadataCache::new(Limits::default());
    for resource in &resources {
        cache.admit(resource).unwrap();
    }
    let complete = cache.resolve(&inputs, vec![]).unwrap();
    let guid = complete.snapshot.separators().next().unwrap().guid.clone();
    let mut changed = false;
    for parsed in &mut cache.facts {
        for descriptor in &mut parsed.descriptors {
            if descriptor.object_guid == guid {
                descriptor.separation = None;
                changed = true;
            }
        }
    }
    assert!(changed);
    let full = cache.resolve(&inputs, vec![]).unwrap_err();
    let runtime = cache.resolve_runtime(&inputs, vec![]).unwrap_err();
    assert!(matches!(full, DbError::Data(ref text) if text.contains("unsupported settings")));
    assert_eq!(format!("{full:?}"), format!("{runtime:?}"));
    for parsed in &mut cache.facts {
        parsed
            .descriptors
            .retain(|descriptor| descriptor.object_guid != guid);
    }
    assert!(cache.resource_guids.contains(&guid));
    let full = cache.resolve(&inputs, vec![]).unwrap_err();
    let runtime = cache.resolve_runtime(&inputs, vec![]).unwrap_err();
    assert!(
        matches!(full, DbError::Data(ref text) if text.contains("no supported own descriptor"))
    );
    assert_eq!(format!("{full:?}"), format!("{runtime:?}"));
}
