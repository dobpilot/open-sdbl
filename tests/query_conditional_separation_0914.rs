//! Контракт глобального условия разделения по нативным контролям 8.3.27.2342.
mod support;
use open_sdbl::metadata::{MetadataSnapshot, SeparatedDataUse};
use open_sdbl::query::{
    Backend, CompileOptions, MsSqlBackend, ParameterValue, PostgresBackend, QueryCompiler,
    QueryParameter, SessionParameters,
};
use support::{guid, separators_snapshot, with_descriptors};
const CONDITION: &str = "cc906f05-5ea6-43cb-a084-c4ee51ca4bd5";
fn snapshot(mode: SeparatedDataUse) -> MetadataSnapshot {
    with_descriptors(separators_snapshot(), |descriptors| {
        for descriptor in descriptors {
            if let Some(settings) = &mut descriptor.separation {
                settings.conditional_separation = Some(guid(CONDITION));
                settings.mode = mode;
            }
        }
    })
}
fn control<B: Backend>(backend: B, mode: SeparatedDataUse) {
    let snapshot = snapshot(mode);
    let compiler = QueryCompiler::new(&snapshot, backend);
    let sources = [
        "ВЫБРАТЬ Ссылка ИЗ Справочник.Товары",
        "ВЫБРАТЬ Значение ИЗ Константа.ОсновнойТовар",
        "ВЫБРАТЬ ОсновнойТовар ИЗ Константы",
        "ВЫБРАТЬ Поставщик.Артикул ИЗ Справочник.Товары",
    ];
    let mut session = SessionParameters::new();
    for source in sources {
        assert!(compiler.prepare(source).is_ok(), "{source}");
        let missing = compiler.compile(source).unwrap_err();
        assert_eq!(
            missing.missing_separation_condition(),
            Some(&guid(CONDITION))
        );
        assert_eq!(missing.missing_parameter_name(), None);
        assert!(!missing.is_missing_session_parameter());
        session.set_separation_condition(guid(CONDITION), false);
        let query = compiler
            .compile_with(source, &CompileOptions::new().session(&session))
            .unwrap();
        assert!(query.sql.contains(" = 0"), "{}", query.sql);
        assert_eq!(session.iter().count(), 0);
        session.set_separation_condition(guid(CONDITION), true);
        let missing = compiler
            .compile_with(source, &CompileOptions::new().session(&session))
            .unwrap_err();
        assert!(missing.is_missing_session_parameter());
        assert_eq!(
            missing.missing_parameter_name(),
            Some("ИспользованиеРазделителя")
        );
        session.set(QueryParameter::new(
            "ИспользованиеРазделителя",
            ParameterValue::Boolean(false),
        ));
        let disabled = compiler.compile_with(source, &CompileOptions::new().session(&session));
        // Прежний источник Константы требует единственную область даже
        // для Shared; false-condition выше задаёт её пустым предикатом.
        let refusal =
            mode == SeparatedDataUse::Independent || source == "ВЫБРАТЬ ОсновнойТовар ИЗ Константы";
        assert_eq!(disabled.is_err(), refusal, "{source}: {disabled:?}");
        session.set(QueryParameter::new(
            "ИспользованиеРазделителя",
            ParameterValue::Boolean(true),
        ));
        session.set(QueryParameter::new(
            "ЗначениеРазделителя",
            ParameterValue::Number {
                unscaled: 7,
                scale: 0,
            },
        ));
        let active = compiler
            .compile_with(source, &CompileOptions::new().session(&session))
            .unwrap();
        assert!(active.sql.contains(" = 7"), "{}", active.sql);
        session.set_separation_condition(guid(CONDITION), false);
        let empty = compiler
            .compile_with(source, &CompileOptions::new().session(&session))
            .unwrap();
        assert!(empty.sql.contains(" = 0"), "{}", empty.sql);
        assert!(!empty.sql.contains(" = 7"), "{}", empty.sql);
        session = SessionParameters::new();
    }
    assert!(compiler.compile("ВЫБРАТЬ 1 КАК Число").is_ok());
}
#[test]
fn independent_postgres() {
    control(PostgresBackend, SeparatedDataUse::Independent);
}
#[test]
fn independent_mssql() {
    control(
        MsSqlBackend::new(2000).unwrap(),
        SeparatedDataUse::Independent,
    );
}
#[test]
fn shared_postgres() {
    control(PostgresBackend, SeparatedDataUse::IndependentAndShared);
}
#[test]
fn shared_mssql() {
    control(
        MsSqlBackend::new(2000).unwrap(),
        SeparatedDataUse::IndependentAndShared,
    );
}
#[test]
fn conditions_are_separate_replaceable_values() {
    let id = guid(CONDITION);
    let mut session = SessionParameters::new();
    assert_eq!(session.separation_condition(&id), None);
    session.set_separation_condition(id.clone(), false);
    assert_eq!(session.separation_condition(&id), Some(false));
    session.set_separation_condition(id.clone(), true);
    assert_eq!(session.separation_condition(&id), Some(true));
    assert!(session.remove_separation_condition(&id));
    assert!(!session.remove_separation_condition(&id));
    assert_eq!(session.separation_condition(&id), None);
}

#[test]
fn native_config_and_xml_condition_references_agree() {
    let native = include_bytes!("fixtures/conditional-separation-0914/condition.deflate");
    let acquired = open_sdbl::metadata::parse_config_descriptors(
        "03f9d2d0-eae4-4098-a70e-4eb569b11fd9",
        native,
    )
    .unwrap();
    let condition = guid("9893e2d6-f3f8-4d73-bb06-19bf26d216ab");
    assert_eq!(
        acquired[0]
            .separation
            .as_ref()
            .unwrap()
            .conditional_separation,
        Some(condition.clone())
    );
    let attribute_xml =
        include_str!("fixtures/conditional-separation-0914/xml/НаименованиеЯзык1.xml");
    let constant_xml =
        include_str!("fixtures/conditional-separation-0914/xml/ЗначениеОпцииПробы.xml");
    assert!(
        attribute_xml
            .contains("<ConditionalSeparation>Constant.ЗначениеОпцииПробы</ConditionalSeparation>")
    );
    assert!(constant_xml.contains(&format!("<Constant uuid=\"{condition}\">")));
    assert!(constant_xml.contains("<v8:Type>xs:boolean</v8:Type>"));
    let no_condition = include_bytes!("fixtures/conditional-separation-0914/no-condition.deflate");
    let acquired = open_sdbl::metadata::parse_config_descriptors(
        "03f9d2d0-eae4-4098-a70e-4eb569b11fd9",
        no_condition,
    )
    .unwrap();
    assert_eq!(
        acquired[0]
            .separation
            .as_ref()
            .unwrap()
            .conditional_separation,
        None
    );
    let demo = include_bytes!("fixtures/conditional-separation-0914/demo.deflate");
    let acquired =
        open_sdbl::metadata::parse_config_descriptors("6df2bb92-558c-4453-9de4-e4176e8f93dc", demo)
            .unwrap();
    assert_eq!(
        acquired[0]
            .separation
            .as_ref()
            .unwrap()
            .conditional_separation,
        Some(guid(CONDITION))
    );
}
fn typed_snapshot(mode: SeparatedDataUse, tag: &str) -> MetadataSnapshot {
    let base = snapshot(mode);
    let mut schema = base.schema().clone();
    for table in &mut schema.tables {
        for column in &mut table.columns {
            if column.name == "Fld56" || column.name == "Fld57" {
                for kind in &mut column.types {
                    kind.tag = tag.to_owned();
                }
            }
        }
    }
    open_sdbl::metadata::resolve_metadata(
        base.db_names().clone(),
        base.descriptors().to_vec(),
        schema,
        base.live_tables().to_vec(),
    )
    .snapshot
}
#[test]
fn conditional_string_empty_and_unsupported_empty_types() {
    for mode in [
        SeparatedDataUse::Independent,
        SeparatedDataUse::IndependentAndShared,
    ] {
        let mut session = SessionParameters::new();
        session.set_separation_condition(guid(CONDITION), false);
        for tag in ["S", "L", "T", "R"] {
            let snapshot = typed_snapshot(mode, tag);
            let options = CompileOptions::new().session(&session);
            let source = "ВЫБРАТЬ Ссылка ИЗ Справочник.Товары";
            for result in [
                QueryCompiler::new(&snapshot, PostgresBackend).compile_with(source, &options),
                QueryCompiler::new(&snapshot, MsSqlBackend::new(2000).unwrap())
                    .compile_with(source, &options),
            ] {
                if tag == "S" {
                    let sql = result.unwrap().sql;
                    assert!(sql.contains(" = ''") || sql.contains(" = N''"), "{sql}");
                } else {
                    assert!(
                        result
                            .unwrap_err()
                            .message()
                            .contains("unsupported conditional empty value")
                    );
                }
            }
        }
    }
}

fn virtual_snapshot(base: MetadataSnapshot) -> MetadataSnapshot {
    use open_sdbl::metadata::{DbNameEntry, LiveColumn, resolve_metadata};
    let id = guid("a1b2c3d4-0009-4000-8000-000000000009");
    let mut entries = base.db_names().entries().to_vec();
    entries.extend([
        DbNameEntry {
            guid: id.clone(),
            alias: "Fld".to_owned(),
            number: 999,
        },
        DbNameEntry {
            guid: id.clone(),
            alias: "DataSeparationUse".to_owned(),
            number: 1000,
        },
    ]);
    let mut descriptors = base.descriptors().to_vec();
    let source = snapshot(SeparatedDataUse::Independent);
    let mut separator = source
        .descriptors()
        .iter()
        .find(|value| value.object_guid == guid("a1b2c3d4-0003-4000-8000-000000000003"))
        .unwrap()
        .clone();
    separator.object_guid = id;
    descriptors.push(separator);
    descriptors.extend(
        source
            .descriptors()
            .iter()
            .filter(|value| {
                [
                    guid("a1b2c3d4-0001-4000-8000-000000000001"),
                    guid("a1b2c3d4-0002-4000-8000-000000000002"),
                ]
                .contains(&value.object_guid)
            })
            .cloned(),
    );
    let mut schema = base.schema().clone();
    for table in &mut schema.tables {
        table
            .columns
            .push(support::schema_column("Fld999", "N", None));
    }
    let mut tables = base.live_tables().to_vec();
    for table in &mut tables {
        table.columns.push(LiveColumn {
            name: "_fld999".to_owned(),
            data_type: "numeric(7,0)".to_owned(),
        });
    }
    let serialized = format!(
        "{{{},{}}}",
        entries.len(),
        entries
            .iter()
            .map(|entry| format!("{{{},\"{}\",{}}}", entry.guid, entry.alias, entry.number))
            .collect::<Vec<_>>()
            .join(",")
    );
    let db_names =
        open_sdbl::metadata::parse_db_names(&support::stored_deflate(serialized.as_bytes()))
            .unwrap();
    resolve_metadata(db_names, descriptors, schema, tables).snapshot
}
#[test]
fn condition_filters_virtual_base_reads_for_both_backends() {
    for (base, source) in [
        (
            support::information_register_snapshot(),
            "ВЫБРАТЬ Период ИЗ РегистрСведений.Prices.СрезПоследних()",
        ),
        (
            support::accumulation_register_snapshot(),
            "ВЫБРАТЬ КоличествоОстаток ИЗ РегистрНакопления.Остатки.Остатки()",
        ),
        (
            support::accumulation_register_snapshot(),
            "ВЫБРАТЬ КоличествоОборот ИЗ РегистрНакопления.Остатки.Обороты()",
        ),
    ] {
        let snapshot = virtual_snapshot(base);
        let mut session = SessionParameters::new();
        let missing = QueryCompiler::new(&snapshot, PostgresBackend)
            .compile(source)
            .unwrap_err();
        assert_eq!(
            missing.missing_separation_condition(),
            Some(&guid(CONDITION))
        );
        session.set_separation_condition(guid(CONDITION), false);
        let options = CompileOptions::new().session(&session);
        for result in [
            QueryCompiler::new(&snapshot, PostgresBackend).compile_with(source, &options),
            QueryCompiler::new(&snapshot, MsSqlBackend::new(2000).unwrap())
                .compile_with(source, &options),
        ] {
            let sql = result.unwrap().sql;
            assert!(sql.contains(" = 0"), "{sql}");
            assert!(sql.contains("_fld999") || sql.contains("_Fld999"), "{sql}");
        }
    }
}

#[test]
fn unknown_named_separator_settings_do_not_become_an_absent_condition() {
    let snapshot = with_descriptors(separators_snapshot(), |descriptors| {
        for descriptor in descriptors {
            if descriptor.separation.is_some() {
                descriptor.separation = None;
            }
        }
    });
    let source = "ВЫБРАТЬ Ссылка ИЗ Справочник.Товары";
    assert!(
        QueryCompiler::new(&snapshot, PostgresBackend)
            .prepare(source)
            .is_ok()
    );
    for result in [
        QueryCompiler::new(&snapshot, PostgresBackend).compile(source),
        QueryCompiler::new(&snapshot, MsSqlBackend::new(2000).unwrap()).compile(source),
    ] {
        let error = result.unwrap_err();
        assert!(error.message().contains("unsupported separation settings"));
        assert_eq!(error.missing_separation_condition(), None);
        assert!(!error.is_missing_session_parameter());
    }
}

#[test]
fn demo_condition_target_is_the_acquired_boolean_constant() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/demo");
    let snapshot = support::demo_resolved_at(&root).snapshot;
    let id = guid(CONDITION);
    let object = snapshot
        .objects()
        .iter()
        .find(|object| object.guid == id)
        .unwrap();
    assert_eq!(
        object.kind,
        Some(open_sdbl::metadata::MetadataKind::Constant)
    );
    assert_eq!(
        object.name.as_deref(),
        Some("ИспользоватьРазделениеПоОбластямДанных")
    );
    assert_eq!(object.physical_table.as_deref(), Some("_Const1835"));
    let descriptor = snapshot
        .descriptors()
        .iter()
        .find(|descriptor| descriptor.object_guid == id)
        .unwrap();
    assert_eq!(
        descriptor
            .declared_types
            .as_ref()
            .unwrap()
            .iter()
            .map(|kind| kind.tag.as_str())
            .collect::<Vec<_>>(),
        ["B"]
    );
}
