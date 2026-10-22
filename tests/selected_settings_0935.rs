#[path = "support/mod.rs"]
mod support;

use open_sdbl::metadata::{LiveColumn, LiveTable, resolve_selected_metadata};
use open_sdbl::query::{CommonSettingsSeparation, ParameterValue, PostgresBackend, QueryCompiler};

#[test]
fn native_settings_declarations_and_separator_mappings_survive_selection() {
    let full = support::separators_snapshot();
    let live = ["_commonsettings", "_systemsettings", "_unrelated"]
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
        .collect::<Vec<_>>();
    let whole = open_sdbl::metadata::resolve_metadata(
        full.db_names().clone(),
        full.descriptors().to_vec(),
        full.schema().clone(),
        live.clone(),
    )
    .snapshot;
    let selected = resolve_selected_metadata(
        full.db_names(),
        full.descriptors().to_vec(),
        vec![],
        vec![],
        full.schema(),
        &live,
    )
    .snapshot;
    assert!(selected.live_table("_unrelated").is_none());
    assert!(selected.schema().table("RepSettings").is_none());
    for name in ["CommonSettings", "SystemSettings"] {
        assert_eq!(selected.schema().table(name), whole.schema().table(name));
    }
    let scope = [56, 57]
        .into_iter()
        .map(|number| {
            let guid = full
                .db_names()
                .entries()
                .iter()
                .find(|entry| entry.alias == "Fld" && entry.number == number)
                .unwrap()
                .guid
                .clone();
            CommonSettingsSeparation::Active {
                guid,
                value: ParameterValue::Number {
                    unscaled: 7,
                    scale: 0,
                },
            }
        })
        .collect::<Vec<_>>();
    for system in [false, true] {
        let compile = |snapshot: &open_sdbl::metadata::MetadataSnapshot,
                       states: &[CommonSettingsSeparation]| {
            let compiler = QueryCompiler::new(snapshot, PostgresBackend);
            if system {
                compiler.compile_system_settings_load("u", "o", "k", states)
            } else {
                compiler.compile_common_settings_load("u", "o", "k", states)
            }
        };
        assert_eq!(
            compile(&selected, &scope).unwrap(),
            compile(&whole, &scope).unwrap()
        );
        assert!(compile(&selected, &scope[..1]).is_err());
        let mut duplicate = scope.clone();
        duplicate.push(scope[0].clone());
        assert!(compile(&selected, &duplicate).is_err());
    }
}
