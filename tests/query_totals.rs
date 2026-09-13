//! `ИТОГИ … ПО …`: the rows of the platform's linear traversal.

mod support;

use support::*;

use open_sdbl::metadata::MetadataSnapshot;
use open_sdbl::query::{
    Backend, ColumnKind, CompileOptions, CompiledQuery, MsSqlBackend, PostgresBackend,
    QueryCompiler, QueryDiagnostic, QueryDiagnosticKind,
};

fn compile<B: Backend>(
    snapshot: &MetadataSnapshot,
    backend: B,
    source: &str,
    level: bool,
) -> Result<CompiledQuery, QueryDiagnostic> {
    QueryCompiler::new(snapshot, backend)
        .compile_with(source, &CompileOptions::new().totals_level(level))
}

fn postgres(snapshot: &MetadataSnapshot, source: &str) -> CompiledQuery {
    compile(snapshot, PostgresBackend, source, false)
        .unwrap_or_else(|error| panic!("{source}: {error}"))
}

fn assert_contains(sql: &str, needle: &str) {
    assert!(sql.contains(needle), "expected {needle:?} in\n{sql}");
}

const QUERY: &str =
    "ВЫБРАТЬ Т.Номенклатура КАК Номенклатура, Т.Период КАК Период, Т.Количество КАК Количество
     ИЗ РегистрНакопления.Остатки КАК Т
     УПОРЯДОЧИТЬ ПО Количество УБЫВ
     ИТОГИ СУММА(Количество), МАКСИМУМ(Период) КАК Период ПО ОБЩИЕ, Номенклатура;";

#[test]
fn renders_overall_and_one_level_in_traversal_order() {
    let snapshot = accumulation_register_snapshot();
    let compiled = postgres(&snapshot, QUERY);
    let sql = &compiled.sql;
    assert!(sql.starts_with("WITH \"__totals_rows\" AS (SELECT \"Номенклатура\", \"Период\", \"Количество\", ROW_NUMBER() OVER (ORDER BY \"__order_1\" DESC) AS \"__rn\" FROM (SELECT"), "{sql}");
    assert_contains(
        sql,
        "\"Т\".\"_fld55\" AS \"__order_1\" FROM \"_accumrg53\" AS \"Т\") AS \"__totals_source\")",
    );
    assert!(
        !sql.contains("ORDER BY \"__src\""),
        "inner ORDER BY must be dropped:\n{sql}"
    );
    assert_contains(
        sql,
        "SELECT CAST(NULL AS bytea) AS \"Номенклатура\", MAX(\"Период\") AS \"Период\", SUM(\"Количество\") AS \"Количество\", 0 AS \"__level\", 0 AS \"__g1\", 0 AS \"__rn\" FROM \"__totals_rows\" HAVING COUNT(*) > 0",
    );
    assert_contains(
        sql,
        "UNION ALL SELECT \"Номенклатура\", MAX(\"Период\") AS \"Период\", SUM(\"Количество\") AS \"Количество\", 1 AS \"__level\", MIN(MIN(\"__rn\")) OVER (PARTITION BY \"Номенклатура\") AS \"__g1\", 0 AS \"__rn\" FROM \"__totals_rows\" GROUP BY \"Номенклатура\"",
    );
    assert_contains(
        sql,
        "UNION ALL SELECT \"Номенклатура\", \"Период\", \"Количество\", 2 AS \"__level\", MIN(\"__rn\") OVER (PARTITION BY \"Номенклатура\") AS \"__g1\", \"__rn\" FROM \"__totals_rows\") AS \"__totals\" ORDER BY \"__g1\", CASE WHEN \"__level\" <= 1 THEN 0 ELSE 1 END, \"__rn\"",
    );
    assert!(sql.contains("SELECT \"Номенклатура\", \"Период\", \"Количество\" FROM ("));
    assert_eq!(compiled.columns.len(), 3);

    let with_level = compile(&snapshot, PostgresBackend, QUERY, true).unwrap();
    assert_contains(
        &with_level.sql,
        "SELECT \"Номенклатура\", \"Период\", \"Количество\", \"__level\" FROM (",
    );
    assert_eq!(with_level.columns.len(), 4);
    assert_eq!(with_level.columns[3].label, "__level");
    assert!(matches!(
        with_level.columns[3].kind,
        ColumnKind::Number { .. }
    ));

    let mssql = compile(&snapshot, MsSqlBackend::new(2000).unwrap(), QUERY, false).unwrap();
    assert!(
        mssql.sql.starts_with("WITH [__totals_rows] AS (SELECT"),
        "{}",
        mssql.sql
    );
    assert_contains(&mssql.sql, "CAST(NULL AS varbinary) AS [Номенклатура]");
    assert_contains(
        &mssql.sql,
        "ORDER BY [__g1], CASE WHEN [__level] <= 1 THEN 0 ELSE 1 END, [__rn]",
    );
}

#[test]
fn nests_levels_and_counts_the_period_as_text_when_needed() {
    let snapshot = accumulation_register_snapshot();
    let two = postgres(
        &snapshot,
        "ВЫБРАТЬ Т.Номенклатура, Т.Период КАК Период, Т.Количество ИЗ РегистрНакопления.Остатки КАК Т
         ИТОГИ СУММА(Количество) * 2 КАК Количество ПО Номенклатура, Период;",
    );
    assert_contains(
        &two.sql,
        "ROW_NUMBER() OVER (ORDER BY (SELECT 1)) AS \"__rn\"",
    );
    assert_contains(
        &two.sql,
        "(SUM(\"Количество\") * 2) AS \"Количество\", 0 AS \"__level\", MIN(MIN(\"__rn\")) OVER (PARTITION BY \"Номенклатура\") AS \"__g1\", 0 AS \"__g2\", 0 AS \"__rn\" FROM \"__totals_rows\" GROUP BY \"Номенклатура\"",
    );
    assert_contains(
        &two.sql,
        "1 AS \"__level\", MIN(MIN(\"__rn\")) OVER (PARTITION BY \"Номенклатура\") AS \"__g1\", MIN(MIN(\"__rn\")) OVER (PARTITION BY \"Период\") AS \"__g2\", 0 AS \"__rn\" FROM \"__totals_rows\" GROUP BY \"Номенклатура\", \"Период\"",
    );
    assert_contains(
        &two.sql,
        "2 AS \"__level\", MIN(\"__rn\") OVER (PARTITION BY \"Номенклатура\") AS \"__g1\", MIN(\"__rn\") OVER (PARTITION BY \"Период\") AS \"__g2\", \"__rn\" FROM \"__totals_rows\") AS \"__totals\" ORDER BY \"__g1\", CASE WHEN \"__level\" <= 0 THEN 0 ELSE 1 END, \"__g2\", CASE WHEN \"__level\" <= 1 THEN 0 ELSE 1 END, \"__rn\"",
    );
    assert!(!two.sql.contains("HAVING COUNT(*) > 0"));

    let count_into_text = postgres(
        &support::snapshot(),
        "ВЫБРАТЬ Code КАК Код ИЗ Справочник.OpenSdblMetadataProbe ИТОГИ КОЛИЧЕСТВО(Код) ПО ОБЩИЕ;",
    );
    assert_contains(&count_into_text.sql, "(COUNT(\"Код\"))::text AS \"Код\"");
}

#[test]
fn accepts_union_top_parameters_and_periods() {
    let snapshot = accumulation_register_snapshot();
    let union = postgres(
        &snapshot,
        "ВЫБРАТЬ Т.Номенклатура КАК Н, Т.Количество КАК К ИЗ РегистрНакопления.Остатки КАК Т
         ОБЪЕДИНИТЬ ВСЕ
         ВЫБРАТЬ Т.Номенклатура, Т.Количество * 10 ИЗ РегистрНакопления.Остатки КАК Т
         УПОРЯДОЧИТЬ ПО К
         ИТОГИ СУММА(К) ПО Н;",
    );
    assert_contains(
        &union.sql,
        "ROW_NUMBER() OVER (ORDER BY \"К\") AS \"__rn\" FROM ((SELECT",
    );
    assert_contains(&union.sql, "UNION ALL (SELECT");
    assert!(!union.sql.contains(") ORDER BY 2 ASC"), "{}", union.sql);

    let top = postgres(
        &snapshot,
        "ВЫБРАТЬ ПЕРВЫЕ 3 Т.Номенклатура КАК Н, Т.Количество КАК К ИЗ РегистрНакопления.Остатки КАК Т
         УПОРЯДОЧИТЬ ПО К УБЫВ
         ИТОГИ СУММА(К) ПО ОБЩИЕ, Н;",
    );
    assert_contains(
        &top.sql,
        "AS \"__order_1\" FROM \"_accumrg53\" AS \"Т\" ORDER BY \"__order_1\" DESC LIMIT 3) AS \"__totals_source\"",
    );

    let periods = postgres(
        &snapshot,
        "ВЫБРАТЬ Т.Период КАК Период, Т.Количество КАК Количество ИЗ РегистрНакопления.Остатки КАК Т
         ИТОГИ СУММА(Количество) ПО Период ПЕРИОДАМИ(МЕСЯЦ, ДАТАВРЕМЯ(2024, 1, 1), ДАТАВРЕМЯ(2024, 12, 31)) КАК Месяц;",
    );
    assert_contains(&periods.sql, "GROUP BY \"Период\"");
}

#[test]
fn reports_totals_diagnostics() {
    let snapshot = accumulation_register_snapshot();
    for (query, kind, message) in [
        (
            "ВЫБРАТЬ Т.Количество КАК К ПОМЕСТИТЬ ВТ ИЗ РегистрНакопления.Остатки КАК Т ИТОГИ СУММА(К) ПО ОБЩИЕ;",
            QueryDiagnosticKind::Syntax,
            "defines the temporary table",
        ),
        (
            "ВЫБРАТЬ Х.К ИЗ (ВЫБРАТЬ Т.Количество КАК К ИЗ РегистрНакопления.Остатки КАК Т ИТОГИ СУММА(К) ПО ОБЩИЕ) КАК Х;",
            QueryDiagnosticKind::UnsupportedFeature,
            "inside the nested query",
        ),
        (
            "ВЫБРАТЬ Т.Количество КАК К ИЗ РегистрНакопления.Остатки КАК Т ИТОГИ СУММА(К) ПО Номенклатура;",
            QueryDiagnosticKind::UnsupportedFeature,
            "must name a result column",
        ),
        (
            "ВЫБРАТЬ Т.Количество КАК К ИЗ РегистрНакопления.Остатки КАК Т ИТОГИ СУММА(К) КАК Итого ПО ОБЩИЕ;",
            QueryDiagnosticKind::Syntax,
            "names no result column",
        ),
        (
            "ВЫБРАТЬ Т.Количество КАК К ИЗ РегистрНакопления.Остатки КАК Т ИТОГИ СУММА(К) * 2 ПО ОБЩИЕ;",
            QueryDiagnosticKind::Syntax,
            "cannot determine the result column",
        ),
        (
            "ВЫБРАТЬ Т.Номенклатура КАК Н, Т.Количество КАК К ИЗ РегистрНакопления.Остатки КАК Т ИТОГИ СУММА(К) ПО Н ИЕРАРХИЯ;",
            QueryDiagnosticKind::UnsupportedFeature,
            "HIERARCHY totals are not supported yet",
        ),
        (
            "ВЫБРАТЬ Т.Номенклатура КАК Н, Т.Количество КАК К ИЗ РегистрНакопления.Остатки КАК Т ИТОГИ СУММА(К) ПО Н ПЕРИОДАМИ(МЕСЯЦ);",
            QueryDiagnosticKind::Syntax,
            "must be a date column",
        ),
        (
            "ВЫБРАТЬ Т.Период КАК П, Т.Количество КАК К ИЗ РегистрНакопления.Остатки КАК Т ИТОГИ СУММА(К) ПО П ПЕРИОДАМИ(МЕСЯЦ, 1);",
            QueryDiagnosticKind::Syntax,
            "bounds must be DATETIME literals",
        ),
        (
            "ВЫБРАТЬ Т.Период КАК П, Т.Количество КАК К ИЗ РегистрНакопления.Остатки КАК Т ИТОГИ СУММА(П) ПО ОБЩИЕ;",
            QueryDiagnosticKind::Syntax,
            "of another kind",
        ),
        (
            "ВЫБРАТЬ Т.Количество КАК К ИЗ РегистрНакопления.Остатки КАК Т ИТОГИ ПО;",
            QueryDiagnosticKind::Syntax,
            "expected field name",
        ),
    ] {
        let error = compile(&snapshot, PostgresBackend, query, false).unwrap_err();
        assert_eq!(error.kind(), kind, "{query}: {error}");
        assert!(error.message().contains(message), "{query}: {error}");
    }
}

#[test]
fn keeps_the_temporary_table_with_list_ahead_of_the_totals_cte() {
    let snapshot = accumulation_register_snapshot();
    let batch = postgres(
        &snapshot,
        "ВЫБРАТЬ Т.Номенклатура КАК Н, Т.Количество КАК К ПОМЕСТИТЬ ВТ ИЗ РегистрНакопления.Остатки КАК Т;
         ВЫБРАТЬ Х.Н КАК Н, Х.К КАК К ИЗ ВТ КАК Х ИТОГИ СУММА(К) ПО ОБЩИЕ, Н;",
    );
    assert!(
        batch.sql.starts_with("WITH \"vt1\" AS (SELECT"),
        "{}",
        batch.sql
    );
    assert_contains(&batch.sql, "), \"__totals_rows\" AS (SELECT");
}
