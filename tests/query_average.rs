//! The `СРЕДНЕЕ`/`AVG` aggregate.

mod support;

use support::*;

use open_sdbl::metadata::MetadataSnapshot;
use open_sdbl::query::{
    Backend, ColumnKind, CompiledQuery, MsSqlBackend, PostgresBackend, QueryCompiler,
    QueryDiagnostic, QueryDiagnosticKind,
};

fn compile<B: Backend>(
    snapshot: &MetadataSnapshot,
    backend: B,
    source: &str,
) -> Result<CompiledQuery, QueryDiagnostic> {
    QueryCompiler::new(snapshot, backend).compile(source)
}

fn assert_contains(sql: &str, needle: &str) {
    assert!(sql.contains(needle), "expected {needle:?} in\n{sql}");
}

#[test]
fn averages_fields_and_expressions_in_grouped_branches() {
    let snapshot = accumulation_register_snapshot();
    let query = "ВЫБРАТЬ Номенклатура, СРЕДНЕЕ(Количество) КАК Среднее, СРЕДНЕЕ(Количество * 2) КАК Удвоенное
         ИЗ РегистрНакопления.Остатки
         СГРУППИРОВАТЬ ПО Номенклатура
         ИМЕЮЩИЕ СРЕДНЕЕ(Количество) > 1;";
    let postgres = compile(&snapshot, PostgresBackend, query).unwrap();
    assert_contains(&postgres.sql, "AVG(\"__src\".\"_fld55\") AS \"Среднее\"");
    assert_contains(
        &postgres.sql,
        "AVG((\"__src\".\"_fld55\" * 2)) AS \"Удвоенное\"",
    );
    assert_contains(&postgres.sql, "HAVING (AVG(\"__src\".\"_fld55\") > 1)");
    assert_eq!(postgres.columns[1].label, "Среднее");
    assert!(matches!(
        postgres.columns[1].kind,
        ColumnKind::Number { .. }
    ));
    assert!(matches!(
        postgres.columns[2].kind,
        ColumnKind::Number { .. }
    ));

    let mssql = compile(&snapshot, MsSqlBackend::new(2000).unwrap(), query).unwrap();
    assert_contains(&mssql.sql, "AVG([__src].[_fld55]) AS [Среднее]");
    assert_contains(&mssql.sql, "HAVING (AVG([__src].[_fld55]) > 1)");
}

#[test]
fn averages_a_whole_table_and_nested_values() {
    let snapshot = accumulation_register_snapshot();
    let whole = compile(
        &snapshot,
        PostgresBackend,
        "SELECT AVG(Количество) AS A, COUNT(*) AS N FROM AccumulationRegister.Остатки;",
    )
    .unwrap();
    assert_contains(&whole.sql, "AVG(\"__src\".\"_fld55\") AS \"A\"");
    assert!(!whole.sql.contains("GROUP BY"));

    let nested = compile(
        &snapshot,
        PostgresBackend,
        "ВЫБРАТЬ СРЕДНЕЕ(Т.Ч) КАК Ср ИЗ (ВЫБРАТЬ 1 КАК Ч ОБЪЕДИНИТЬ ВСЕ ВЫБРАТЬ 2) КАК Т;",
    )
    .unwrap();
    assert_contains(&nested.sql, "AVG(\"Т\".\"Ч\") AS \"Ср\"");
    assert!(matches!(nested.columns[0].kind, ColumnKind::Number { .. }));
}

#[test]
fn refuses_distinct_wildcard_and_source_free_averages() {
    let snapshot = accumulation_register_snapshot();
    for (query, message) in [
        (
            "ВЫБРАТЬ СРЕДНЕЕ(РАЗЛИЧНЫЕ Количество) ИЗ РегистрНакопления.Остатки;",
            "supported only by COUNT",
        ),
        (
            "ВЫБРАТЬ СРЕДНЕЕ(*) ИЗ РегистрНакопления.Остатки;",
            "supported only by COUNT",
        ),
        ("ВЫБРАТЬ СРЕДНЕЕ(1);", "requires FROM"),
        (
            "ВЫБРАТЬ СРЕДНЕЕ(СУММА(Количество)) ИЗ РегистрНакопления.Остатки;",
            "aggregate",
        ),
    ] {
        let error = compile(&snapshot, PostgresBackend, query).unwrap_err();
        assert_eq!(
            error.kind(),
            QueryDiagnosticKind::UnsupportedFeature,
            "{query}: {error}"
        );
        assert!(error.message().contains(message), "{query}: {error}");
    }
}
