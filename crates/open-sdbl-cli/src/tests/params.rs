//! Tests of the `params` module.

use super::{
    ParameterCommand, ParameterStore, apply_parameter_command, parse_parameter_command,
    parse_parameter_literal,
};
use open_sdbl::metadata::{
    ConfigDescriptor, Guid, LiveColumn, LiveTable, MetadataSnapshot, SchemaStorage, parse_db_names,
    resolve_metadata,
};
use open_sdbl::query::{ColumnKind, ParameterColumn};
use open_sdbl::query::{ParameterDate, ParameterValue, QueryParameter};
use std::str::FromStr;

pub(crate) fn enumeration_snapshot() -> MetadataSnapshot {
    let owner = Guid::from_str("c8b21fea-1e3d-4ae9-8719-7ff4db08af97").unwrap();
    let value = Guid::from_str("d2f8bde9-fadd-4be8-9022-249e3a1ac4b9").unwrap();
    let db_names = parse_db_names(&crate::hex_test_support::hex(
        "ab36d4a94eb64832324c4b4dd4354c354ed135494cb5d4b53037b4d4354f4b33494932b0484cb334d75172cd2bcd55d2b1b4acad0500",
    ))
    .unwrap();
    let descriptor = |object_guid: Guid, name: &str, enumeration_value: bool| ConfigDescriptor {
        resource_guid: owner.clone(),
        object_guid,
        marker: "1".to_owned(),
        name: name.to_owned(),
        synonyms: Vec::new(),
        comment: None,
        field_purpose: None,
        enumeration_value,
        separation: None,
        reference_types: Vec::new(),
        object_reference_type: None,
        balance: None,
        chart_of_accounts: None,
    };
    let status = descriptor(value, "Статус", true);
    let object = descriptor(owner.clone(), "бит_ВидыСтатусовОбъектов", false);
    resolve_metadata(
        db_names,
        vec![object, status],
        SchemaStorage {
            tables: Vec::new(),
            anomalies: Vec::new(),
        },
        vec![LiveTable {
            name: "_enum99".to_owned(),
            columns: vec![
                LiveColumn {
                    name: "_idrref".to_owned(),
                    data_type: "bytea".to_owned(),
                },
                LiveColumn {
                    name: "_enumorder".to_owned(),
                    data_type: "numeric".to_owned(),
                },
            ],
            indexes: Vec::new(),
        }],
    )
    .snapshot
}

#[test]
fn parses_every_literal_form() {
    let snapshot = enumeration_snapshot();
    let parse = |text: &str| parse_parameter_literal(text, &snapshot).unwrap();
    assert_eq!(
        parse("15.50"),
        ParameterValue::Number {
            unscaled: 1550,
            scale: 2
        }
    );
    assert_eq!(
        parse("-7"),
        ParameterValue::Number {
            unscaled: -7,
            scale: 0
        }
    );
    assert_eq!(
        parse("\"a\"\"b\""),
        ParameterValue::String("a\"b".to_owned())
    );
    assert_eq!(parse("ИСТИНА"), ParameterValue::Boolean(true));
    assert_eq!(parse("false"), ParameterValue::Boolean(false));
    assert_eq!(parse("NULL"), ParameterValue::Null);
    assert_eq!(
        parse("ДАТАВРЕМЯ(2024, 1, 2, 3, 4, 5)"),
        ParameterValue::Date(ParameterDate::new(2024, 1, 2, 3, 4, 5).unwrap())
    );
    assert_eq!(parse("0x0A0b"), ParameterValue::Binary(vec![0x0a, 0x0b]));
    assert!(matches!(
        parse("ЗНАЧЕНИЕ(Перечисление.бит_ВидыСтатусовОбъектов.Статус)"),
        ParameterValue::Reference { .. }
    ));
    assert!(matches!(
        parse("VALUE(Enum.бит_ВидыСтатусовОбъектов.EmptyRef)"),
        ParameterValue::Reference { id, .. } if id == [0; 16]
    ));
    assert_eq!(
        parse("(1, \"x\", NULL)"),
        ParameterValue::List(vec![
            ParameterValue::Number {
                unscaled: 1,
                scale: 0
            },
            ParameterValue::String("x".to_owned()),
            ParameterValue::Null,
        ])
    );
    assert_eq!(parse("()"), ParameterValue::List(Vec::new()));
}

#[test]
fn parses_table_literals() {
    let snapshot = enumeration_snapshot();
    let parse = |text: &str| parse_parameter_literal(text, &snapshot).unwrap();
    let table = parse("ТАБЛИЦА(Код КАК СТРОКА, Количество КАК ЧИСЛО)((\"A\", 1), (\"B\", 2))");
    assert_eq!(
        table,
        ParameterValue::Table {
            columns: vec![
                ParameterColumn::new("Код", ColumnKind::String { length: None }),
                ParameterColumn::new(
                    "Количество",
                    ColumnKind::Number {
                        precision: None,
                        scale: None
                    }
                ),
            ],
            rows: vec![
                vec![
                    ParameterValue::String("A".to_owned()),
                    ParameterValue::Number {
                        unscaled: 1,
                        scale: 0
                    }
                ],
                vec![
                    ParameterValue::String("B".to_owned()),
                    ParameterValue::Number {
                        unscaled: 2,
                        scale: 0
                    }
                ],
            ],
        }
    );
    assert_eq!(
        parse("TABLE(Ссылка КАК ЛЮБАЯССЫЛКА, Дата КАК ДАТА)()"),
        ParameterValue::Table {
            columns: vec![
                ParameterColumn::new(
                    "Ссылка",
                    ColumnKind::Reference {
                        targets: Vec::new(),
                        runtime_typed: true
                    }
                ),
                ParameterColumn::new("Дата", ColumnKind::DateTime),
            ],
            rows: Vec::new(),
        }
    );
    let untyped = parse_parameter_literal("ТАБЛИЦА(Код)()", &snapshot).unwrap_err();
    assert!(untyped.to_string().contains("needs a kind"), "{untyped}");
}

#[test]
fn rejects_malformed_literals() {
    let snapshot = enumeration_snapshot();
    for text in [
        "",
        "1 2",
        "ДАТАВРЕМЯ(2024, 13, 1)",
        "ДАТАВРЕМЯ(2024)",
        "((1))",
        "Поле",
        "0x1",
        "ЗНАЧЕНИЕ(Перечисление.бит_ВидыСтатусовОбъектов.Нет)",
        "ЗНАЧЕНИЕ(Справочник.Нет.Значение)",
    ] {
        assert!(
            parse_parameter_literal(text, &snapshot).is_err(),
            "{text:?} should be rejected"
        );
    }
}

#[test]
fn stores_lists_and_filters_parameters_by_reference() {
    let snapshot = enumeration_snapshot();
    let mut store = ParameterStore::new();
    let mut session = ParameterStore::new();
    assert_eq!(
        parse_parameter_command("\\set Период ДАТАВРЕМЯ(2024, 1, 1)"),
        Some(ParameterCommand::Set {
            name: "Период",
            literal: "ДАТАВРЕМЯ(2024, 1, 1)"
        })
    );
    assert_eq!(
        parse_parameter_command("\\set &Лимит 10"),
        Some(ParameterCommand::Set {
            name: "Лимит",
            literal: "10"
        })
    );
    assert_eq!(
        parse_parameter_command("\\set"),
        Some(ParameterCommand::SetUsage)
    );
    assert_eq!(
        parse_parameter_command("\\set x"),
        Some(ParameterCommand::SetUsage)
    );
    assert_eq!(
        parse_parameter_command("\\params"),
        Some(ParameterCommand::List)
    );
    assert_eq!(
        parse_parameter_command("\\unset Лимит"),
        Some(ParameterCommand::Unset { name: "Лимит" })
    );
    assert_eq!(parse_parameter_command("\\dt"), None);

    let output = apply_parameter_command(
        &mut store,
        &mut session,
        ParameterCommand::Set {
            name: "Период",
            literal: "ДАТАВРЕМЯ(2024, 1, 1)",
        },
        &snapshot,
    )
    .unwrap();
    assert_eq!(output, "Parameter Период set [DateTime].\n");
    apply_parameter_command(
        &mut store,
        &mut session,
        ParameterCommand::Set {
            name: "Список",
            literal: "(1, 2)",
        },
        &snapshot,
    )
    .unwrap();
    apply_parameter_command(
        &mut store,
        &mut session,
        ParameterCommand::Set {
            name: "период",
            literal: "ДАТАВРЕМЯ(2025, 1, 1)",
        },
        &snapshot,
    )
    .unwrap();
    assert_eq!(
        store.listing(),
        "период  ДАТАВРЕМЯ(2025, 1, 1)  [DateTime]\nСписок  (1, 2)  [List[2]]\n"
    );
    assert_eq!(store.names(), ["период", "Список"]);

    let referenced = store.values_for("ВЫБРАТЬ 1 ГДЕ &ПЕРИОД > 0;");
    assert_eq!(referenced.len(), 1);
    assert_eq!(referenced[0].name(), "период");
    assert!(store.values_for("ВЫБРАТЬ 1;").is_empty());
    assert!(store.values_for("ВЫБРАТЬ \"unterminated").is_empty());

    assert_eq!(
        apply_parameter_command(
            &mut store,
            &mut session,
            ParameterCommand::Unset {
                name: "СПИСОК"
            },
            &snapshot
        )
        .unwrap(),
        "Parameter СПИСОК removed.\n"
    );
    assert!(
        apply_parameter_command(
            &mut store,
            &mut session,
            ParameterCommand::Unset {
                name: "Список"
            },
            &snapshot
        )
        .is_err()
    );
    assert!(
        apply_parameter_command(
            &mut store,
            &mut session,
            ParameterCommand::SetUsage,
            &snapshot
        )
        .is_err()
    );
    assert_eq!(
        apply_parameter_command(&mut store, &mut session, ParameterCommand::List, &snapshot)
            .unwrap(),
        "период  ДАТАВРЕМЯ(2025, 1, 1)  [DateTime]\n"
    );
}

#[test]
fn stores_session_parameters_for_every_query() {
    let snapshot = enumeration_snapshot();
    let mut store = ParameterStore::new();
    let mut session = ParameterStore::new();
    assert_eq!(
        parse_parameter_command("\\session Орг = \"HQ\""),
        Some(ParameterCommand::Session {
            name: "Орг",
            literal: "\"HQ\""
        })
    );
    assert_eq!(
        parse_parameter_command("\\session &Лимит 10"),
        Some(ParameterCommand::Session {
            name: "Лимит",
            literal: "10"
        })
    );
    assert_eq!(
        parse_parameter_command("\\session"),
        Some(ParameterCommand::SessionList)
    );
    assert_eq!(
        parse_parameter_command("\\session CLEAR"),
        Some(ParameterCommand::SessionClear)
    );
    assert_eq!(
        parse_parameter_command("\\session Орг"),
        Some(ParameterCommand::SessionUsage)
    );
    assert_eq!(
        parse_parameter_command("\\session 1 = 2"),
        Some(ParameterCommand::SessionUsage)
    );

    let output = apply_parameter_command(
        &mut store,
        &mut session,
        ParameterCommand::Session {
            name: "Орг",
            literal: "\"HQ\"",
        },
        &snapshot,
    )
    .unwrap();
    assert_eq!(output, "Session parameter Орг set [String].\n");
    apply_parameter_command(
        &mut store,
        &mut session,
        ParameterCommand::Session {
            name: "орг",
            literal: "\"Branch\"",
        },
        &snapshot,
    )
    .unwrap();
    assert!(store.names().is_empty());
    assert_eq!(session.names(), ["орг"]);
    let parameters = session.session_parameters();
    assert_eq!(
        parameters.get("ОРГ").map(QueryParameter::value),
        Some(&ParameterValue::String("Branch".to_owned()))
    );
    assert_eq!(
        apply_parameter_command(
            &mut store,
            &mut session,
            ParameterCommand::SessionList,
            &snapshot
        )
        .unwrap(),
        "орг  \"Branch\"  [String]\n"
    );
    assert!(
        apply_parameter_command(
            &mut store,
            &mut session,
            ParameterCommand::SessionUsage,
            &snapshot
        )
        .is_err()
    );
    assert_eq!(
        apply_parameter_command(
            &mut store,
            &mut session,
            ParameterCommand::SessionClear,
            &snapshot
        )
        .unwrap(),
        "Session parameters cleared.\n"
    );
    assert!(session.session_parameters().is_empty());
    assert_eq!(
        apply_parameter_command(
            &mut store,
            &mut session,
            ParameterCommand::SessionList,
            &snapshot
        )
        .unwrap(),
        "No session parameters set.\n"
    );
}
