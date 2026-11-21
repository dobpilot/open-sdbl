#[path = "../../../tests/support/mod.rs"]
mod support;

use open_sdbl::metadata::{ColumnType, DbNames, MetadataKind, Synonym, resolve_metadata};
use open_sdbl::query::{
    AccessDecision, CompileOptions, MsSqlBackend, PostgresBackend, PrepareOptions,
    PresentationExpression, PresentationPlan, QueryCompiler, QueryDiagnosticKind,
    RestrictionTarget,
};
use open_sdbl_db::presentation::default_presentation_plan;

const ACCOUNTING_QUERY: &str = "ВЫБРАТЬ РАЗЛИЧНЫЕ
    Номенклатура.ВидСтавкиНДС КАК ВидСтавкиНДС,
    УНИКАЛЬНЫЙИДЕНТИФИКАТОР(Номенклатура.ВидСтавкиНДС) КАК ВидСтавкиНДС_UUID,
    ПРЕДСТАВЛЕНИЕ(Номенклатура.ВидСтавкиНДС) КАК ВидСтавкиНДС_Представление,
    ПРЕДСТАВЛЕНИЕССЫЛКИ(Номенклатура.ВидСтавкиНДС) КАК ВидСтавкиНДС_ПредставлениеСсылки,
    ТИПЗНАЧЕНИЯ(Номенклатура.ВидСтавкиНДС)
    ИЗ Справочник.Номенклатура КАК Номенклатура";

fn snapshot() -> open_sdbl::metadata::MetadataSnapshot {
    let base = support::enumeration_value_snapshot();
    let mut descriptors = base.descriptors().to_vec();
    let value = descriptors
        .iter_mut()
        .find(|d| d.enumeration_value)
        .unwrap();
    value.synonyms.push(Synonym {
        language: "ru".to_owned(),
        text: " Статус 'готов' ".to_owned(),
    });
    resolve_metadata(
        base.db_names().clone(),
        descriptors,
        base.schema().clone(),
        base.live_tables().to_vec(),
    )
    .snapshot
}

#[test]
fn enumeration_presentations_use_metadata_and_safe_sql_on_both_backends() {
    let snapshot = snapshot();
    let object = snapshot
        .object_id(MetadataKind::Enumeration, "бит_ВидыСтатусовОбъектов")
        .unwrap();
    let plan = default_presentation_plan(&snapshot, object);
    let PresentationExpression::Enumeration(entries) = &plan.expression else {
        panic!("expected enumeration plan");
    };
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].1, "Статус 'готов'");
    assert_eq!(
        entries[0].0,
        snapshot
            .predefined_value(object, "Статус")
            .unwrap()
            .guid
            .to_1c_bytes()
    );
    let source = "SELECT PRESENTATION(Ссылка), REFPRESENTATION(Ссылка) FROM Enumeration.бит_ВидыСтатусовОбъектов";
    let postgres = QueryCompiler::new(&snapshot, PostgresBackend)
        .prepare(source)
        .unwrap()
        .compile(&snapshot, std::slice::from_ref(&plan))
        .unwrap();
    let mssql = QueryCompiler::new(&snapshot, MsSqlBackend::new(0).unwrap())
        .prepare(source)
        .unwrap()
        .compile(&snapshot, std::slice::from_ref(&plan))
        .unwrap();
    for sql in [&postgres.sql, &mssql.sql] {
        assert_eq!(sql.matches("Статус ''готов''").count(), 2, "{sql}");
        assert!(sql.contains("9022249e3a1ac4b94be8faddd2f8bde9"), "{sql}");
        assert!(
            sql.contains("ELSE '' END") || sql.contains("ELSE N'' END"),
            "{sql}"
        );
    }
    for sql in [
        QueryCompiler::new(&snapshot, PostgresBackend)
            .compile_presentation_lookup(&plan, &[entries[0].0])
            .unwrap()
            .sql,
        QueryCompiler::new(&snapshot, MsSqlBackend::new(0).unwrap())
            .compile_presentation_lookup(&plan, &[entries[0].0])
            .unwrap()
            .sql,
    ] {
        assert!(sql.contains("Статус ''готов''"), "{sql}");
    }
}

#[test]
fn enum_presentation_uses_name_when_russian_synonym_is_missing() {
    let snapshot = support::enumeration_value_snapshot();
    let object = snapshot
        .object_id(MetadataKind::Enumeration, "бит_ВидыСтатусовОбъектов")
        .unwrap();
    let plan = default_presentation_plan(&snapshot, object);
    assert!(
        matches!(plan.expression, PresentationExpression::Enumeration(values) if values[0].1 == "Статус")
    );
}

#[test]
fn enumeration_plan_rejects_foreign_and_duplicate_values() {
    let snapshot = snapshot();
    let object = snapshot
        .object_id(MetadataKind::Enumeration, "бит_ВидыСтатусовОбъектов")
        .unwrap();
    let plan = default_presentation_plan(&snapshot, object);
    let PresentationExpression::Enumeration(entries) = &plan.expression else {
        panic!("expected enumeration plan");
    };
    for entries in [
        vec![([42; 16], "foreign".to_owned())],
        vec![entries[0].clone(), entries[0].clone()],
        vec![entries[0].clone(); 4097],
    ] {
        let invalid = PresentationPlan {
            expression: PresentationExpression::Enumeration(entries),
            ..plan.clone()
        };
        let error = QueryCompiler::new(&snapshot, PostgresBackend)
            .compile_presentation_lookup(&invalid, &[[1; 16]])
            .unwrap_err();
        assert_eq!(error.kind(), QueryDiagnosticKind::PresentationPlan);
    }
    let missing_reference_field = PresentationPlan {
        fields: vec![],
        ..plan.clone()
    };
    assert_eq!(
        QueryCompiler::new(&snapshot, PostgresBackend)
            .compile_presentation_lookup(&missing_reference_field, &[[1; 16]])
            .unwrap_err()
            .kind(),
        QueryDiagnosticKind::PresentationPlan
    );
    let catalog = support::snapshot();
    let catalog_object = catalog
        .object_id(MetadataKind::Catalog, "OpenSdblMetadataProbe")
        .unwrap();
    let invalid_target = PresentationPlan {
        object: catalog_object,
        fields: vec![open_sdbl::metadata::FieldId::Standard(
            open_sdbl::metadata::StandardFieldId::Id,
        )],
        expression: PresentationExpression::Enumeration(Vec::new()),
    };
    assert_eq!(
        QueryCompiler::new(&catalog, PostgresBackend)
            .compile_presentation_lookup(&invalid_target, &[[1; 16]])
            .unwrap_err()
            .kind(),
        QueryDiagnosticKind::PresentationPlan
    );
}

#[test]
fn deferred_enumeration_lookup_keeps_restricted_access_decisions() {
    let snapshot = snapshot();
    let object = snapshot
        .object_id(MetadataKind::Enumeration, "бит_ВидыСтатусовОбъектов")
        .unwrap();
    let plan = default_presentation_plan(&snapshot, object);
    let value = snapshot
        .predefined_value(object, "Статус")
        .unwrap()
        .guid
        .to_1c_bytes();
    let compiler = QueryCompiler::new(&snapshot, PostgresBackend);
    let prepared = compiler
        .prepare_with_options(
            "SELECT REFPRESENTATION(Ссылка) FROM Enumeration.бит_ВидыСтатусовОбъектов",
            &PrepareOptions::new().restricted(),
        )
        .unwrap();
    assert!(
        prepared
            .compile_presentation_lookup(&snapshot, &plan, &[value], &CompileOptions::new())
            .is_err()
    );
    let denied = [AccessDecision::denied(RestrictionTarget {
        object,
        table_part: None,
    })];
    let lookup = prepared
        .compile_presentation_lookup(
            &snapshot,
            &plan,
            &[value],
            &CompileOptions::new().decisions(&denied),
        )
        .unwrap();
    assert!(lookup.sql.contains("FALSE"), "{}", lookup.sql);
}

#[test]
fn enumeration_value_expression_with_source_is_deferred() {
    let snapshot = snapshot();
    let source = "SELECT PRESENTATION(VALUE(Enumeration.бит_ВидыСтатусовОбъектов.Статус)) FROM Enumeration.бит_ВидыСтатусовОбъектов";
    let prepared = QueryCompiler::new(&snapshot, PostgresBackend)
        .prepare(source)
        .unwrap();
    let compiled = prepared.compile(&snapshot, &[]).unwrap();
    assert_eq!(compiled.deferred_presentations, [0]);
    for function in ["PRESENTATION", "REFPRESENTATION"] {
        let source =
            format!("SELECT {function}(VALUE(Enumeration.бит_ВидыСтатусовОбъектов.Статус))");
        let postgres = QueryCompiler::new(&snapshot, PostgresBackend)
            .compile(&source)
            .unwrap();
        let mssql = QueryCompiler::new(&snapshot, MsSqlBackend::new(0).unwrap())
            .compile(&source)
            .unwrap();
        for compiled in [postgres, mssql] {
            assert_eq!(compiled.deferred_presentations, [0]);
            assert!(compiled.sql.contains("9022249e3a1ac4b94be8faddd2f8bde9"));
        }
    }
}

#[test]
fn enumeration_field_uses_the_enumeration_target_join() {
    let base = support::snapshot();
    let enumeration = snapshot();
    let mut entries = base.db_names().entries().to_vec();
    entries.extend_from_slice(enumeration.db_names().entries());
    let mut descriptors = base.descriptors().to_vec();
    descriptors.extend_from_slice(enumeration.descriptors());
    let mut schema = base.schema().clone();
    let field = schema.tables[0]
        .columns
        .iter_mut()
        .find(|column| column.name == "Fld54")
        .unwrap();
    field.types = vec![ColumnType {
        tag: "R".to_owned(),
        reference_target: Some("Enum99".to_owned()),
    }];
    let mut tables = base.live_tables().to_vec();
    tables.extend_from_slice(enumeration.live_tables());
    let snapshot =
        resolve_metadata(DbNames::from_entries(entries), descriptors, schema, tables).snapshot;
    let source = "SELECT PRESENTATION(ProbeAttribute), REFPRESENTATION(ProbeAttribute) FROM Catalog.OpenSdblMetadataProbe";
    let prepared = QueryCompiler::new(&snapshot, PostgresBackend)
        .prepare(source)
        .unwrap();
    assert_eq!(prepared.presentation_request().targets.len(), 1);
    let object = prepared.presentation_request().targets[0].object;
    let plan = default_presentation_plan(&snapshot, object);
    let query = prepared.compile(&snapshot, &[plan]).unwrap();
    assert_eq!(query.sql.matches("LEFT JOIN").count(), 1, "{}", query.sql);
    assert!(query.sql.contains("\"_enum99\""), "{}", query.sql);
    assert!(query.sql.contains("Статус ''готов''"), "{}", query.sql);
}

#[test]
fn accounting_vat_rate_example_compiles_with_presentations_and_uuid() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/buh");
    let snapshot = support::demo_resolved_at(&path).snapshot;
    let query = ACCOUNTING_QUERY;
    let prepared = QueryCompiler::new(&snapshot, PostgresBackend)
        .prepare(query)
        .unwrap();
    let plans: Vec<_> = prepared
        .presentation_request()
        .targets
        .iter()
        .map(|target| default_presentation_plan(&snapshot, target.object))
        .collect();
    let mut session = open_sdbl::query::SessionParameters::new();
    for separator in snapshot.separators() {
        if let Some(condition) = separator
            .separation
            .as_ref()
            .and_then(|s| s.conditional_separation.as_ref())
        {
            session.set_separation_condition(condition.clone(), false);
        }
    }
    let options = CompileOptions::new()
        .presentations(&plans)
        .session(&session);
    let postgres = prepared.compile_with(&snapshot, &options).unwrap();
    let mssql = QueryCompiler::new(&snapshot, MsSqlBackend::new(0).unwrap())
        .prepare(query)
        .unwrap()
        .compile_with(&snapshot, &options)
        .unwrap();
    for compiled in [postgres, mssql] {
        use open_sdbl::query::ColumnKind;
        assert_eq!(compiled.columns.len(), 5);
        assert!(
            matches!(compiled.columns[0].kind, ColumnKind::Reference { ref targets, runtime_typed: false } if targets.len() == 1)
        );
        assert_eq!(compiled.columns[1].kind, ColumnKind::Uuid);
        assert!(matches!(
            compiled.columns[2].kind,
            ColumnKind::String { .. }
        ));
        assert!(matches!(
            compiled.columns[3].kind,
            ColumnKind::String { .. }
        ));
        assert_eq!(compiled.columns[4].kind, ColumnKind::Type);
        assert!(compiled.sql.contains("SELECT DISTINCT"));
        assert!(compiled.sql.contains("Без НДС"));
        assert_eq!(compiled.sql.matches("LEFT JOIN").count(), 1);
    }
}

#[test]
#[ignore = "requires a disposable PostgreSQL database and psql"]
fn accounting_vat_rate_example_executes_on_postgres() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/buh");
    let snapshot = support::demo_resolved_at(&path).snapshot;
    let prepared = QueryCompiler::new(&snapshot, PostgresBackend)
        .prepare(ACCOUNTING_QUERY)
        .unwrap();
    let plans: Vec<_> = prepared
        .presentation_request()
        .targets
        .iter()
        .map(|target| default_presentation_plan(&snapshot, target.object))
        .collect();
    let mut session = open_sdbl::query::SessionParameters::new();
    for separator in snapshot.separators() {
        if let Some(condition) = separator
            .separation
            .as_ref()
            .and_then(|s| s.conditional_separation.as_ref())
        {
            session.set_separation_condition(condition.clone(), false);
        }
    }
    let query = prepared
        .compile_with(
            &snapshot,
            &CompileOptions::new()
                .presentations(&plans)
                .session(&session),
        )
        .unwrap();
    let script = format!(
        "CREATE TEMP TABLE _reference347 (_fld9146rref bytea, _fld2331 numeric); \
         CREATE TEMP TABLE _enum1427 (_idrref bytea); \
         INSERT INTO _reference347 VALUES \
         (decode('8dd5045b5453ad404d39e2cad8ec7206', 'hex'), 0), \
         (decode('b2f95e56162faf8a4898756342df267e', 'hex'), 0); \
         INSERT INTO _enum1427 SELECT _fld9146rref FROM _reference347; {};",
        query.sql
    );
    let output = std::process::Command::new("psql")
        .args([
            "-X",
            "-A",
            "-t",
            "-q",
            "-v",
            "ON_ERROR_STOP=1",
            "-c",
            &script,
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = String::from_utf8(output.stdout).unwrap();
    let mut presentations = Vec::new();
    for row in output.lines() {
        let fields: Vec<_> = row.split('|').collect();
        assert_eq!(fields.len(), 5, "{row}");
        assert!(fields[0].starts_with("\\x"));
        assert_eq!(fields[1].len(), 36);
        assert_eq!(fields[2], fields[3]);
        assert_eq!(fields[4], "\\x0800000593");
        presentations.push(fields[2]);
    }
    presentations.sort_unstable();
    assert_eq!(presentations, ["Без НДС", "Общая"]);
}

#[test]
#[ignore = "requires a disposable PostgreSQL database and psql"]
fn enumeration_presentations_execute_on_postgres() {
    let snapshot = snapshot();
    let object = snapshot
        .object_id(MetadataKind::Enumeration, "бит_ВидыСтатусовОбъектов")
        .unwrap();
    let plan = default_presentation_plan(&snapshot, object);
    let source = "SELECT PRESENTATION(Ссылка), REFPRESENTATION(Ссылка) FROM Enumeration.бит_ВидыСтатусовОбъектов";
    let query = QueryCompiler::new(&snapshot, PostgresBackend)
        .prepare(source)
        .unwrap()
        .compile(&snapshot, std::slice::from_ref(&plan))
        .unwrap();
    let constant = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile("SELECT PRESENTATION(VALUE(Enumeration.бит_ВидыСтатусовОбъектов.Статус))")
        .unwrap();
    let value = snapshot
        .predefined_value(object, "Статус")
        .unwrap()
        .guid
        .to_1c_bytes();
    let lookup = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile_presentation_lookup(&plan, &[value])
        .unwrap();
    let script = format!(
        "CREATE TEMP TABLE _enum99 (_idrref bytea, _enumorder numeric); \
         INSERT INTO _enum99 VALUES \
         (decode('9022249e3a1ac4b94be8faddd2f8bde9', 'hex'), 1), \
         (decode('ffffffffffffffffffffffffffffffff', 'hex'), 2), (NULL, 3); {}; {}; {};",
        query.sql, constant.sql, lookup.sql
    );
    let output = std::process::Command::new("psql")
        .args([
            "-X",
            "-A",
            "-t",
            "-q",
            "-v",
            "ON_ERROR_STOP=1",
            "-c",
            &script,
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rows = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        rows.lines().collect::<Vec<_>>(),
        [
            "Статус 'готов'|Статус 'готов'",
            "|",
            "|",
            "\\x000000639022249e3a1ac4b94be8faddd2f8bde9",
            "\\x9022249e3a1ac4b94be8faddd2f8bde9|Статус 'готов'",
        ]
    );
}
