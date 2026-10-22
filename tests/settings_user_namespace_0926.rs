use open_sdbl::metadata::{
    DbNameEntry, DbNames, Guid, LiveColumn, LiveTable, MetadataSnapshot, SchemaStorage,
    resolve_metadata,
};
use open_sdbl::query::{CommonSettingsSeparation, ParameterValue, PostgresBackend, QueryCompiler};

fn guid() -> Guid {
    "03f9d2d0-eae4-4098-a70e-4eb569b11fd9".parse().unwrap()
}

fn metadata(storage: &str, field_type: &str) -> MetadataSnapshot {
    resolve_metadata(
        DbNames::from_entries(vec![
            DbNameEntry {
                guid: guid(),
                alias: "Fld".into(),
                number: 55,
            },
            DbNameEntry {
                guid: guid(),
                alias: "DataSeparationUse".into(),
                number: 58,
            },
        ]),
        vec![],
        SchemaStorage {
            tables: vec![],
            anomalies: vec![],
        },
        vec![LiveTable {
            name: storage.into(),
            columns: [
                ("_UserID", "mvarchar"),
                ("_ObjectKey", "mvarchar(256)"),
                ("_SettingsKey", "mvarchar"),
                ("_SettingsData", "bytea"),
                ("_Fld55", field_type),
                ("_DataSeparationUse58", "boolean"),
            ]
            .into_iter()
            .map(|(name, data_type)| LiveColumn {
                name: name.into(),
                data_type: data_type.into(),
            })
            .collect(),
            indexes: vec![],
        }],
    )
    .snapshot
}

fn compile(
    metadata: &MetadataSnapshot,
    storage: &str,
    user: &str,
    scope: &[CommonSettingsSeparation],
) -> String {
    let compiler = QueryCompiler::new(metadata, PostgresBackend);
    if storage == "_CommonSettings" {
        compiler.compile_common_settings_load(user, "O'K", "Setting", scope)
    } else {
        compiler.compile_system_settings_load(user, "O'K", "Setting", scope)
    }
    .unwrap()
}

#[test]
fn measured_mvarchar_user_namespace_is_exact_without_changing_other_keys() {
    for storage in ["_CommonSettings", "_SystemSettings"] {
        let metadata = metadata(storage, "mvarchar(150)");
        for (user, literal) in [
            ("ControlAdmin", "'ControlAdmin'"),
            ("controladmin", "'controladmin'"),
            ("", "''"),
            ("GhostUser", "'GhostUser'"),
            ("O'Neil'; SELECT 1; --", "'O''Neil''; SELECT 1; --'"),
        ] {
            let sql = compile(
                &metadata,
                storage,
                user,
                &[CommonSettingsSeparation::Inactive { guid: guid() }],
            );
            assert!(
                sql.contains(&format!(
                    "CAST(\"_UserID\" AS text) COLLATE \"C\" = {literal}"
                )),
                "{sql}"
            );
            assert!(sql.contains(&format!("\"_UserID\" = {literal}")), "{sql}");
            assert!(sql.contains("\"_ObjectKey\" = 'O''K'"), "{sql}");
            assert!(sql.contains("\"_SettingsKey\" = 'Setting'"), "{sql}");
            assert!(!sql.contains("CAST(\"_ObjectKey\""), "{sql}");
            assert!(!sql.contains("CAST(\"_SettingsKey\""), "{sql}");
            assert!(sql.starts_with("SELECT \"_SettingsData\""), "{sql}");
            assert!(sql.ends_with(" LIMIT 2"), "{sql}");
        }
    }
}

#[test]
fn exact_user_filter_retains_complete_string_and_numeric_separator_states() {
    for storage in ["_CommonSettings", "_SystemSettings"] {
        for (field_type, value, literal, empty) in [
            (
                "mvarchar(150)",
                ParameterValue::String("A'B".into()),
                "'A''B'",
                "''",
            ),
            (
                "numeric(10,0)",
                ParameterValue::Number {
                    unscaled: 101,
                    scale: 0,
                },
                "101",
                "0",
            ),
        ] {
            let metadata = metadata(storage, field_type);
            for (state, enabled, stored) in [
                (
                    CommonSettingsSeparation::Active {
                        guid: guid(),
                        value,
                    },
                    "TRUE",
                    literal,
                ),
                (
                    CommonSettingsSeparation::Inactive { guid: guid() },
                    "FALSE",
                    empty,
                ),
                (
                    CommonSettingsSeparation::ConditionDisabled { guid: guid() },
                    "TRUE",
                    empty,
                ),
            ] {
                let sql = compile(&metadata, storage, "controladmin", &[state]);
                assert!(
                    sql.contains("CAST(\"_UserID\" AS text) COLLATE \"C\" = 'controladmin'"),
                    "{sql}"
                );
                assert!(
                    sql.contains(&format!("\"_DataSeparationUse58\" = {enabled}")),
                    "{sql}"
                );
                assert!(sql.contains(&format!("\"_Fld55\" = {stored}")), "{sql}");
            }
        }
    }
}

#[test]
fn retained_native_oracles_distinguish_user_namespaces_for_authorized_profiles() {
    for oracle in [
        include_str!("fixtures/settings-user-namespace-0926/ControlAdmin.oracle"),
        include_str!("fixtures/settings-user-namespace-0926/SettingsDataAdmin.oracle"),
        include_str!("fixtures/settings-user-namespace-0926/SettingsAdmin.oracle"),
    ] {
        for storage in ["common", "system"] {
            let value = |key: &str| {
                oracle
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once('\t')?;
                        (name == key).then_some(value)
                    })
                    .unwrap()
            };
            assert!(value(&format!("{storage}.load.current")).starts_with("ok:"));
            assert_eq!(value(&format!("{storage}.load.lower-current")), "none");
            assert_eq!(
                value(&format!("{storage}.load.empty")),
                format!("ok:{storage}|")
            );
            assert_eq!(
                value(&format!("{storage}.load.ghost")),
                format!("ok:{storage}|GhostUser")
            );
        }
    }
}
