use open_sdbl::metadata::{
    LiveColumn, LiveTable, StorageLayout, parse_config_descriptors, parse_db_names,
    parse_schema_storage, resolve_metadata,
};
use open_sdbl::query::{CommonSettingsSeparation, PostgresBackend, QueryCompiler};
use open_sdbl_db::{
    ConfigResource, ConfigurationMetadataCache, ConfigurationMetadataInputs, DbError, Limits,
    resolve_configuration_resources,
};

fn fixture() -> (ConfigurationMetadataInputs, Vec<ConfigResource>) {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/separators");
    let resources = std::fs::read_dir(root.join("config"))
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
        .collect();
    let live_tables = [
        "_CommonSettings",
        "_SystemSettings",
        "_NotSupportedSettings",
    ]
    .into_iter()
    .map(|name| LiveTable {
        name: name.into(),
        columns: [
            ("_userid", "mvarchar"),
            ("_objectkey", "mvarchar"),
            ("_settingskey", "mvarchar"),
            ("_settingsdata", "bytea"),
            ("_fld56", "numeric(7,0)"),
            ("_fld57", "numeric(7,0)"),
            ("_dataseparationuse59", "boolean"),
            ("_dataseparationuse60", "boolean"),
        ]
        .into_iter()
        .map(|(name, kind)| LiveColumn {
            name: name.into(),
            data_type: kind.into(),
        })
        .collect(),
        indexes: vec![],
    })
    .collect();
    (
        ConfigurationMetadataInputs {
            layout: StorageLayout::MODERN,
            db_names: parse_db_names(&std::fs::read(root.join("db_names.deflate")).unwrap())
                .unwrap(),
            schema: parse_schema_storage(&std::fs::read(root.join("schema_storage.txt")).unwrap())
                .unwrap(),
            live_tables,
        },
        resources,
    )
}

#[test]
fn native_settings_require_missing_descriptors_then_cache_matches_whole_sql() {
    let (inputs, resources) = fixture();
    let mut cache = ConfigurationMetadataCache::new(Limits::default());
    let missing = |result| match result {
        Err(DbError::ConfigurationDescriptorsMissing { objects }) => objects,
        other => panic!("expected strict missing descriptors, got {other:?}"),
    };
    let absent = missing(cache.resolve(&inputs, vec![]));
    assert!(!absent.is_empty());
    assert_eq!(
        absent,
        missing(resolve_configuration_resources(&inputs, &[], vec![]))
    );
    for resource in &resources {
        cache.admit(resource).unwrap();
    }
    let cached = cache.resolve(&inputs, vec![]).unwrap();
    let selected = resolve_configuration_resources(&inputs, &resources, vec![]).unwrap();
    assert_eq!(cached.report, selected.report);
    assert_eq!(
        cached.snapshot.fingerprint(),
        selected.snapshot.fingerprint()
    );
    assert!(
        cached
            .snapshot
            .live_table("_NotSupportedSettings")
            .is_none()
    );
    let whole = resolve_metadata(
        inputs.db_names.clone(),
        resources
            .iter()
            .flat_map(|resource| {
                parse_config_descriptors(&resource.file_name, &resource.compressed).unwrap()
            })
            .collect(),
        inputs.schema.clone(),
        inputs.live_tables.clone(),
    )
    .snapshot;
    let scope = [56, 57]
        .into_iter()
        .map(|number| CommonSettingsSeparation::Inactive {
            guid: inputs
                .db_names
                .entries()
                .iter()
                .find(|entry| entry.alias == "Fld" && entry.number == number)
                .unwrap()
                .guid
                .clone(),
        })
        .collect::<Vec<_>>();
    for snapshot in [&cached.snapshot, &selected.snapshot] {
        for system in [false, true] {
            let compile = |metadata, states: &[CommonSettingsSeparation]| {
                let compiler = QueryCompiler::new(metadata, PostgresBackend);
                if system {
                    compiler.compile_system_settings_load("u", "o", "k", states)
                } else {
                    compiler.compile_common_settings_load("u", "o", "k", states)
                }
            };
            assert_eq!(
                compile(snapshot, &scope).unwrap(),
                compile(&whole, &scope).unwrap()
            );
            assert!(compile(snapshot, &scope[..1]).is_err());
            let mut duplicate = scope.clone();
            duplicate.push(scope[0].clone());
            assert!(compile(snapshot, &duplicate).is_err());
        }
    }
}

#[test]
fn settings_live_facts_keep_prepared_snapshot_validation_strict() {
    let (mut inputs, resources) = fixture();
    let mut cache = ConfigurationMetadataCache::new(Limits::default());
    for resource in &resources {
        cache.admit(resource).unwrap();
    }
    let first = cache.resolve(&inputs, vec![]).unwrap().snapshot;
    let prepared = QueryCompiler::new(&first, PostgresBackend)
        .prepare("SELECT 1 AS N")
        .unwrap();
    inputs
        .live_tables
        .retain(|table| table.name != "_SystemSettings");
    let second = cache.resolve(&inputs, vec![]).unwrap().snapshot;
    assert_ne!(first.fingerprint(), second.fingerprint());
    assert!(prepared.compile(&first, &[]).is_ok());
    assert!(prepared.compile(&second, &[]).is_err());
}
