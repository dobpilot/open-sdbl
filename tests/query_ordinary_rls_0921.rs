//! Контракт обычных остатков по нативным квитанциям `access/native-0910`.

mod support;

use open_sdbl::metadata::{ColumnType, MetadataSnapshot, ObjectId, resolve_metadata};
use open_sdbl::query::{
    AccessDecision, AccessRestriction, CompileOptions, CompiledQuery, MsSqlBackend, ParameterValue,
    PostgresBackend, PrepareOptions, QueryCompiler, QueryDiagnosticKind, QueryParameter,
    SessionParameters, find_metadata_object,
};
use std::process::Command;

fn snapshot() -> MetadataSnapshot {
    let base = support::accumulation_register_snapshot();
    let mut schema = base.schema().clone();
    let mut live = base.live_tables().to_vec();
    for table in &mut schema.tables {
        for field in &mut table.columns {
            if field.name == "Fld54" {
                field.types = vec![ColumnType {
                    tag: "S".to_owned(),
                    reference_target: None,
                }];
            }
        }
    }
    for table in &mut live {
        for field in &mut table.columns {
            if field.name == "_fld54" {
                field.data_type = "character varying(10)".to_owned();
            }
        }
    }
    resolve_metadata(
        base.db_names().clone(),
        base.descriptors().to_vec(),
        schema,
        live,
    )
    .snapshot
}

fn decision(snapshot: &MetadataSnapshot, category: &str) -> AccessDecision {
    let object = find_metadata_object(snapshot, "AccumulationRegister.Остатки").unwrap();
    AccessDecision::restricted(AccessRestriction::new(
        ObjectId::from(&object.guid),
        format!("ГДЕ Номенклатура = \"{category}\""),
    ))
}

fn compile(source: &str, category: &str) -> CompiledQuery {
    let snapshot = snapshot();
    let decisions = [decision(&snapshot, category)];
    QueryCompiler::new(&snapshot, PostgresBackend)
        .prepare_with_options(source, &PrepareOptions::new().restricted())
        .unwrap()
        .compile_with(
            &snapshot,
            &CompileOptions::new()
                .decisions(&decisions)
                .ordinary_row_checks(true),
        )
        .unwrap()
}

#[test]
fn exposes_one_statement_marker_without_changing_logical_column_indices() {
    for projection in [
        "Номенклатура",
        "КоличествоОстаток",
        "Номенклатура, КоличествоОстаток",
    ] {
        let compiled = compile(
            &format!("SELECT {projection} FROM AccumulationRegister.Остатки.Balance()"),
            "a",
        );
        assert_eq!(
            compiled.forbidden_row_column,
            Some(compiled.columns.len() - 1)
        );
        assert_eq!(compiled.service_columns, [compiled.columns.len() - 1]);
        assert_eq!(
            compiled.columns.last().unwrap().kind,
            open_sdbl::query::ColumnKind::Boolean
        );
        assert!(compiled.logical_projection.is_some());
        assert!(compiled.sql.contains("UNION ALL SELECT NULL"));
        assert!(compiled.sql.contains("WHERE EXISTS ("));
        assert!(!compiled.sql.contains(';'));
    }
}

#[test]
fn allowed_keeps_filtered_sql_and_no_forbidden_marker() {
    let compiled = compile(
        "SELECT ALLOWED Номенклатура, КоличествоОстаток FROM AccumulationRegister.Остатки.Balance()",
        "b",
    );
    assert_eq!(compiled.forbidden_row_column, None);
    assert!(!compiled.sql.contains("__forbidden_row"));
}

#[test]
fn refuses_unmeasured_forms_and_backend_explicitly() {
    let snapshot = snapshot();
    let decisions = [decision(&snapshot, "a")];
    let options = CompileOptions::new()
        .decisions(&decisions)
        .ordinary_row_checks(true);
    let compiler = QueryCompiler::new(&snapshot, PostgresBackend);
    for source in [
        "SELECT TOP 1 Номенклатура FROM AccumulationRegister.Остатки.Balance()",
        "SELECT COUNT(*) FROM AccumulationRegister.Остатки.Balance()",
        "SELECT Номенклатура FROM AccumulationRegister.Остатки.Balance() ORDER BY Номенклатура",
        "SELECT Номенклатура FROM AccumulationRegister.Остатки.Balance() WHERE Номенклатура = \"a\"",
        "SELECT Номенклатура FROM AccumulationRegister.Остатки.Balance(DATETIME(2026, 1, 1))",
        "SELECT Номенклатура FROM AccumulationRegister.Остатки.Balance(); SELECT Номенклатура FROM AccumulationRegister.Остатки.Balance()",
        "SELECT b.Номенклатура FROM AccumulationRegister.Остатки.Balance() b LEFT JOIN AccumulationRegister.Остатки.Balance() c ON b.Номенклатура = c.Номенклатура",
    ] {
        let prepared = compiler
            .prepare_with_options(source, &PrepareOptions::new().restricted())
            .unwrap();
        let error = prepared.compile_with(&snapshot, &options).unwrap_err();
        assert_eq!(
            error.kind(),
            QueryDiagnosticKind::Restriction,
            "{source}: {error}"
        );
    }
    let source = "SELECT Номенклатура FROM AccumulationRegister.Остатки.Balance()";
    assert_eq!(
        compiler.compile_with(source, &options).unwrap_err().kind(),
        QueryDiagnosticKind::Restriction
    );
    let mssql = QueryCompiler::new(&snapshot, MsSqlBackend::new(0).unwrap());
    assert_eq!(
        mssql
            .prepare_with_options(source, &PrepareOptions::new().restricted())
            .unwrap()
            .compile_with(&snapshot, &options)
            .unwrap_err()
            .kind(),
        QueryDiagnosticKind::Restriction
    );
    let complex = [decision(&snapshot, "a")];
    let object = complex[0].object();
    let complex = [AccessDecision::restricted(AccessRestriction::new(
        object,
        "Номенклатура = \"a\" ИЛИ Номенклатура = \"b\"",
    ))];
    assert_eq!(
        compiler
            .prepare_with_options(source, &PrepareOptions::new().restricted())
            .unwrap()
            .compile_with(&snapshot, &options.decisions(&complex))
            .unwrap_err()
            .kind(),
        QueryDiagnosticKind::Restriction
    );
}

#[test]
fn binds_current_session_and_manager_parameters_in_both_branches() {
    let snapshot = snapshot();
    let object = find_metadata_object(&snapshot, "AccumulationRegister.Остатки").unwrap();
    let decisions = [AccessDecision::restricted(AccessRestriction::new(
        ObjectId::from(&object.guid),
        "Номенклатура = &Доступ",
    ))];
    let mut session = SessionParameters::new();
    let compiler = QueryCompiler::new(&snapshot, PostgresBackend);
    let prepared = compiler.prepare_with_options("SELECT Номенклатура, КоличествоОстаток FROM AccumulationRegister.Остатки.Balance(, Номенклатура = &Категория)", &PrepareOptions::new().restricted()).unwrap();
    let parameters = [QueryParameter::new(
        "Категория",
        ParameterValue::String("a".to_owned()),
    )];
    for category in ["a", "b"] {
        session.set(QueryParameter::new(
            "Доступ",
            ParameterValue::String(category.to_owned()),
        ));
        let compiled = prepared
            .compile_with(
                &snapshot,
                &CompileOptions::new()
                    .decisions(&decisions)
                    .parameters(&parameters)
                    .session(&session)
                    .ordinary_row_checks(true),
            )
            .unwrap();
        assert!(
            compiled.sql.matches(&format!("'{category}'")).count() >= 2,
            "{}",
            compiled.sql
        );
    }
}

fn psql(port: &str, sql: &str) -> String {
    let output = Command::new("psql")
        .args([
            "-X",
            "-A",
            "-t",
            "-F",
            "\t",
            "-v",
            "ON_ERROR_STOP=1",
            "-h",
            "127.0.0.1",
            "-p",
            port,
            "-U",
            "sdbl_0921",
            "-d",
            "postgres",
            "-c",
            sql,
        ])
        .env("PGPASSFILE", "/dev/null")
        .env_remove("PGPASSWORD")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn executes_measured_paths_on_disposable_postgres_when_requested() {
    let Ok(port) = std::env::var("OPEN_SDBL_ORDINARY_RLS_PGPORT") else {
        eprintln!("disposable PostgreSQL control skipped: OPEN_SDBL_ORDINARY_RLS_PGPORT is unset");
        return;
    };
    psql(
        &port,
        "CREATE TABLE _accumrgt56 (_period timestamp, _fld54 varchar(10), _fld55 numeric(10,2), _splitter numeric); INSERT INTO _accumrgt56 VALUES ('2026-01-01', 'a', 1, 0);",
    );
    for (projection, permitted, forbidden) in [
        ("Номенклатура", "a\tf", "\tt"),
        ("КоличествоОстаток", "1.00\tf", "\tt"),
    ] {
        for (restriction, expected) in [("a", permitted), ("b", forbidden)] {
            let compiled = compile(
                &format!("SELECT {projection} FROM AccumulationRegister.Остатки.Balance()"),
                restriction,
            );
            let result = psql(
                &port,
                &format!("BEGIN READ ONLY; {}; COMMIT;", compiled.sql),
            );
            let rows = result
                .lines()
                .filter(|line| !matches!(*line, "BEGIN" | "COMMIT"))
                .collect::<Vec<_>>()
                .join("\n");
            assert_eq!(
                rows, expected,
                "projection={projection}, restriction={restriction}\n{}",
                compiled.sql
            );
        }
    }
    for (restriction, allowed, filter, expected) in [
        ("a", false, None, "a\t1.00\tf"),
        ("b", false, None, "\t\tt"),
        ("b", true, None, ""),
        ("b", false, Some("a"), "\t\tt"),
        ("b", false, Some("b"), ""),
        ("a", false, Some("a"), "a\t1.00\tf"),
    ] {
        let keyword = if allowed { "ALLOWED " } else { "" };
        let arguments = filter
            .map(|category| format!(", Номенклатура = \"{category}\""))
            .unwrap_or_default();
        let compiled = compile(
            &format!(
                "SELECT {keyword}Номенклатура, КоличествоОстаток FROM AccumulationRegister.Остатки.Balance({arguments})"
            ),
            restriction,
        );
        let sql = format!("BEGIN READ ONLY; {}; COMMIT;", compiled.sql);
        let result = psql(&port, &sql);
        let rows = result
            .lines()
            .filter(|line| !matches!(*line, "BEGIN" | "COMMIT"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            rows, expected,
            "restriction={restriction}, allowed={allowed}, filter={filter:?}\n{}",
            compiled.sql
        );
    }
}
