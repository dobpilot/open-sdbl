use open_sdbl::metadata::{
    DbNameEntry, DbNames, Guid, LiveColumn, LiveTable, MetadataSnapshot, SchemaStorage,
    resolve_metadata,
};
use open_sdbl::query::{
    CommonSettingsSeparation, MsSqlBackend, ParameterValue, PostgresBackend, QueryCompiler,
};

fn guid() -> Guid {
    "03f9d2d0-eae4-4098-a70e-4eb569b11fd9".parse().unwrap()
}

fn fixture_metadata(
    separated: bool,
    mutate: impl FnOnce(&mut Vec<DbNameEntry>, &mut Vec<LiveColumn>),
) -> MetadataSnapshot {
    let mut entries = vec![];
    let mut columns = [
        ("_userid", "mvarchar"),
        ("_objectkey", "mvarchar(256)"),
        ("_settingskey", "mvarchar"),
        ("_settingsdata", "bytea"),
    ]
    .into_iter()
    .map(|(name, data_type)| LiveColumn {
        name: name.into(),
        data_type: data_type.into(),
    })
    .collect::<Vec<_>>();
    if separated {
        entries.extend([
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
        ]);
        columns.extend([
            LiveColumn {
                name: "_fld55".into(),
                data_type: "mvarchar(150)".into(),
            },
            LiveColumn {
                name: "_dataseparationuse58".into(),
                data_type: "boolean".into(),
            },
        ]);
    }
    mutate(&mut entries, &mut columns);
    resolve_metadata(
        DbNames::from_entries(entries),
        vec![],
        SchemaStorage {
            tables: vec![],
            anomalies: vec![],
        },
        vec![LiveTable {
            name: "_systemsettings".into(),
            columns,
            indexes: vec![],
        }],
    )
    .snapshot
}

fn scope(value: Option<&str>) -> CommonSettingsSeparation {
    match value {
        Some(value) => CommonSettingsSeparation::Active {
            guid: guid(),
            value: ParameterValue::String(value.to_owned()),
        },
        None => CommonSettingsSeparation::Inactive { guid: guid() },
    }
}

#[test]
fn complete_active_scope_constrains_user_keys_and_every_separator() {
    let metadata = fixture_metadata(true, |_, _| {});
    let sql = QueryCompiler::new(&metadata, PostgresBackend)
        .compile_system_settings_load("Own", "Object", "Key", &[scope(Some("own-A"))])
        .unwrap();
    for predicate in [
        "\"_userid\" = 'Own'",
        "\"_objectkey\" = 'Object'",
        "\"_settingskey\" = 'Key'",
        "\"_dataseparationuse58\" = TRUE",
        "\"_fld55\" = 'own-A'",
    ] {
        assert!(sql.contains(predicate), "{sql}");
    }
    assert!(sql.starts_with("SELECT \"_settingsdata\""));
    assert!(sql.ends_with(" LIMIT 2"));
}

#[test]
fn inactive_scope_retains_false_and_empty_value_predicates() {
    let metadata = fixture_metadata(true, |_, _| {});
    let sql = QueryCompiler::new(&metadata, PostgresBackend)
        .compile_system_settings_load("", "O", "K", &[scope(None)])
        .unwrap();
    assert!(sql.contains("\"_dataseparationuse58\" = FALSE"));
    assert!(sql.contains("\"_fld55\" = ''"));
    assert!(sql.contains("\"_userid\" = ''"));
}

#[test]
fn missing_duplicate_and_extra_states_fail_closed() {
    let metadata = fixture_metadata(true, |_, _| {});
    let compiler = QueryCompiler::new(&metadata, PostgresBackend);
    assert!(
        compiler
            .compile_system_settings_load("u", "o", "k", &[])
            .is_err()
    );
    assert!(
        compiler
            .compile_system_settings_load("u", "o", "k", &[scope(None), scope(None)])
            .is_err()
    );
    let metadata = fixture_metadata(false, |_, _| {});
    assert!(
        QueryCompiler::new(&metadata, PostgresBackend)
            .compile_system_settings_load("u", "o", "k", &[scope(None)])
            .is_err()
    );
}

#[test]
fn missing_or_ambiguous_dbnames_and_uncovered_flags_fail_closed() {
    for mode in 0..3 {
        let metadata = fixture_metadata(true, |entries, columns| match mode {
            0 => {
                entries.pop();
            }
            1 => {
                entries.push(entries[1].clone());
            }
            _ => columns.push(LiveColumn {
                name: "_dataseparationuse99".into(),
                data_type: "boolean".into(),
            }),
        });
        assert!(
            QueryCompiler::new(&metadata, PostgresBackend)
                .compile_system_settings_load("u", "o", "k", &[scope(None)])
                .is_err()
        );
    }
}

#[test]
fn unsupported_types_and_backend_fail_before_sql() {
    let unsupported = fixture_metadata(true, |_, columns| {
        columns
            .iter_mut()
            .find(|column| column.name == "_fld55")
            .unwrap()
            .data_type = "boolean".into()
    });
    assert!(
        QueryCompiler::new(&unsupported, PostgresBackend)
            .compile_system_settings_load("u", "o", "k", &[scope(None)])
            .is_err()
    );
    let metadata = fixture_metadata(false, |_, _| {});
    assert!(
        QueryCompiler::new(&metadata, MsSqlBackend::new(0).unwrap())
            .compile_system_settings_load("u", "o", "k", &[])
            .is_err()
    );
}

#[test]
fn existing_literal_renderer_escapes_quotes_and_preserves_explicit_empty_user() {
    let metadata = fixture_metadata(false, |_, _| {});
    let compiler = QueryCompiler::new(&metadata, PostgresBackend);
    let sql = compiler
        .compile_system_settings_load("", "O'K", "x'; SELECT 1; --", &[])
        .unwrap();
    assert!(sql.contains("\"_userid\" = ''"));
    assert!(sql.contains("'O''K'"));
    assert!(sql.contains("'x''; SELECT 1; --'"));
    assert!(
        compiler
            .compile_system_settings_load("u", "o\0", "k", &[])
            .is_err()
    );
}

#[test]
fn active_empty_value_remains_distinct_from_an_inactive_separator() {
    let metadata = fixture_metadata(true, |_, _| {});
    let compiler = QueryCompiler::new(&metadata, PostgresBackend);
    let active = compiler
        .compile_system_settings_load("u", "o", "k", &[scope(Some(""))])
        .unwrap();
    let inactive = compiler
        .compile_system_settings_load("u", "o", "k", &[scope(None)])
        .unwrap();
    assert!(active.contains("\"_dataseparationuse58\" = TRUE"));
    assert!(inactive.contains("\"_dataseparationuse58\" = FALSE"));
    assert!(active.contains("\"_fld55\" = ''"));
    assert!(inactive.contains("\"_fld55\" = ''"));
}

#[test]
fn numeric_inactive_scope_has_a_typed_zero() {
    let metadata = fixture_metadata(true, |_, columns| {
        columns
            .iter_mut()
            .find(|column| column.name == "_fld55")
            .unwrap()
            .data_type = "numeric(10,0)".into()
    });
    let sql = QueryCompiler::new(&metadata, PostgresBackend)
        .compile_system_settings_load("u", "o", "k", &[scope(None)])
        .unwrap();
    assert!(sql.contains("\"_fld55\" = 0"));
}

#[test]
fn global_condition_and_inactive_scopes_remain_distinct_for_both_types() {
    for (kind, empty) in [("mvarchar(150)", "''"), ("numeric(10,0)", "0")] {
        let metadata = fixture_metadata(true, |_, columns| {
            columns
                .iter_mut()
                .find(|column| column.name == "_fld55")
                .unwrap()
                .data_type = kind.into()
        });
        let sql = QueryCompiler::new(&metadata, PostgresBackend)
            .compile_system_settings_load(
                "u",
                "o",
                "k",
                &[CommonSettingsSeparation::ConditionDisabled { guid: guid() }],
            )
            .unwrap();
        assert!(sql.contains("\"_dataseparationuse58\" = TRUE"));
        assert!(sql.contains(&format!("\"_fld55\" = {empty}")));
    }
}

#[test]
fn numeric_values_use_the_shared_exact_decimal_renderer() {
    let metadata = fixture_metadata(true, |_, columns| {
        columns
            .iter_mut()
            .find(|column| column.name == "_fld55")
            .unwrap()
            .data_type = "numeric(10,0)".into()
    });
    let compiler = QueryCompiler::new(&metadata, PostgresBackend);
    let sql = compiler
        .compile_system_settings_load(
            "u",
            "o",
            "k",
            &[CommonSettingsSeparation::Active {
                guid: guid(),
                value: ParameterValue::Number {
                    unscaled: 1200,
                    scale: 2,
                },
            }],
        )
        .unwrap();
    assert!(sql.contains("\"_fld55\" = 12.00"));
    assert!(
        compiler
            .compile_system_settings_load("u", "o", "k", &[scope(Some("101"))])
            .is_err()
    );
    let metadata = fixture_metadata(true, |_, _| {});
    assert!(
        QueryCompiler::new(&metadata, PostgresBackend)
            .compile_system_settings_load(
                "u",
                "o",
                "k",
                &[CommonSettingsSeparation::Active {
                    guid: guid(),
                    value: ParameterValue::Boolean(true)
                }]
            )
            .is_err()
    );
}

#[test]
fn system_table_keys_and_payload_require_supported_scalar_columns() {
    for name in ["_userid", "_objectkey", "_settingskey", "_settingsdata"] {
        let missing = fixture_metadata(false, |_, columns| {
            columns.retain(|column| column.name != name)
        });
        assert!(
            QueryCompiler::new(&missing, PostgresBackend)
                .compile_system_settings_load("u", "o", "k", &[])
                .is_err()
        );
        let unsupported = fixture_metadata(false, |_, columns| {
            columns
                .iter_mut()
                .find(|column| column.name == name)
                .unwrap()
                .data_type = "boolean".into()
        });
        assert!(
            QueryCompiler::new(&unsupported, PostgresBackend)
                .compile_system_settings_load("u", "o", "k", &[])
                .is_err()
        );
        let duplicate = fixture_metadata(false, |_, columns| {
            columns.push(
                columns
                    .iter()
                    .find(|column| column.name == name)
                    .unwrap()
                    .clone(),
            )
        });
        assert!(
            QueryCompiler::new(&duplicate, PostgresBackend)
                .compile_system_settings_load("u", "o", "k", &[])
                .is_err()
        );
    }
}

#[test]
fn common_table_cannot_supply_a_missing_system_table() {
    let metadata = resolve_metadata(
        DbNames::from_entries(vec![]),
        vec![],
        SchemaStorage {
            tables: vec![],
            anomalies: vec![],
        },
        vec![LiveTable {
            name: "_commonsettings".into(),
            columns: vec![],
            indexes: vec![],
        }],
    )
    .snapshot;
    assert!(
        QueryCompiler::new(&metadata, PostgresBackend)
            .compile_system_settings_load("u", "o", "k", &[])
            .is_err()
    );
}

#[test]
fn native_payloads_reuse_the_bounded_scalar_and_storage_decoder() {
    use open_sdbl::metadata::{DecodedCommonSetting, decode_common_setting, parse_serialized};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/system-settings-native-1015");
    for (index, expected) in [(0, "29"), (2, "31"), (4, "37")] {
        let bytes =
            std::fs::read(root.join(format!("_systemsettings-{index}.settingsdata"))).unwrap();
        let DecodedCommonSetting::Value(text) = decode_common_setting(&bytes, 1_048_576).unwrap()
        else {
            panic!("скаляр")
        };
        assert_eq!(text, format!("{{\"N\",{expected}}}"));
        assert!(decode_common_setting(&bytes, 1).is_err());
    }
    for index in [1, 3, 5] {
        let bytes =
            std::fs::read(root.join(format!("_systemsettings-{index}.settingsdata"))).unwrap();
        let DecodedCommonSetting::ValueStorage(text) =
            decode_common_setting(&bytes, 1_048_576).unwrap()
        else {
            panic!("хранилище")
        };
        let expected =
            r##"{"#",4238019d-7e49-4fc9-91db-b6b951d5cf8e,{1,{{"S","Probe"},{"N",73}}}}"##;
        assert_eq!(
            parse_serialized(text.as_bytes()).unwrap(),
            parse_serialized(expected.as_bytes()).unwrap()
        );
    }
}

#[test]
fn native_replay_queries_keep_storage_and_scope_independent() {
    for (kind, cases) in [
        (
            "mvarchar(150)",
            vec![
                ("root", scope(None)),
                ("A", scope(Some("A"))),
                ("B", scope(Some("B"))),
            ],
        ),
        (
            "numeric",
            vec![
                ("inactive", scope(None)),
                (
                    "disabled",
                    CommonSettingsSeparation::ConditionDisabled { guid: guid() },
                ),
                (
                    "101",
                    CommonSettingsSeparation::Active {
                        guid: guid(),
                        value: ParameterValue::Number {
                            unscaled: 101,
                            scale: 0,
                        },
                    },
                ),
                (
                    "202",
                    CommonSettingsSeparation::Active {
                        guid: guid(),
                        value: ParameterValue::Number {
                            unscaled: 202,
                            scale: 0,
                        },
                    },
                ),
            ],
        ),
    ] {
        let metadata = fixture_metadata(true, |_, columns| {
            columns
                .iter_mut()
                .find(|column| column.name == "_fld55")
                .unwrap()
                .data_type = kind.into()
        });
        let compiler = QueryCompiler::new(&metadata, PostgresBackend);
        for (case, state) in cases {
            for key in if kind == "numeric" {
                vec!["conditional-shared", "absent"]
            } else {
                vec!["scalar", "absent", "storage"]
            } {
                let object = if kind == "numeric" {
                    "open-bsl-system-settings-numeric-control-1015"
                } else {
                    "open-bsl-system-settings-control-1015"
                };
                let sql = compiler
                    .compile_system_settings_load(
                        "OpenBslSettingsAdmin",
                        object,
                        key,
                        std::slice::from_ref(&state),
                    )
                    .unwrap();
                let common = compiler.compile_common_settings_load(
                    "OpenBslSettingsAdmin",
                    object,
                    key,
                    std::slice::from_ref(&state),
                );
                // В этой фикстуре приобретена только системная таблица.
                assert!(common.is_err());
                println!("NATIVE_SQL\t{kind}\t{case}\t{key}\t{sql}");
                assert!(sql.contains("FROM \"_systemsettings\""));
                assert!(!sql.contains("_commonsettings"));
            }
        }
    }
}

#[test]
fn generated_queries_read_the_four_distinct_native_numeric_payloads() {
    use open_sdbl::metadata::{DecodedCommonSetting, decode_common_setting};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/system-settings-native-1015");
    for (case, expected) in [("inactive", 23), ("disabled", 20), ("101", 21), ("202", 22)] {
        let bytes =
            std::fs::read(root.join(format!("replay-{case}-conditional-shared.settingsdata")))
                .unwrap();
        let DecodedCommonSetting::Value(value) = decode_common_setting(&bytes, 1_048_576).unwrap()
        else {
            panic!("скаляр")
        };
        assert_eq!(value, format!("{{\"N\",{expected}}}"));
    }
}
