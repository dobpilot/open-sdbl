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
    assert!(sql.starts_with("WITH \"__totals_rows\" AS (SELECT \"__totals_source\".\"Номенклатура\" AS \"Номенклатура\", \"__totals_source\".\"Период\" AS \"Период\", \"__totals_source\".\"Количество\" AS \"Количество\", ROW_NUMBER() OVER (ORDER BY \"__order_1\" DESC) AS \"__rn\" FROM (SELECT"), "{sql}");
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
        "SELECT CAST(NULL AS bytea) AS \"Номенклатура\", MAX(\"Период\") AS \"Период\", SUM(\"Количество\") AS \"Количество\", 0 AS \"__level\", 0 AS \"__g1\", 0 AS \"__f1\", 0 AS \"__rn\" FROM \"__totals_rows\" HAVING COUNT(*) > 0",
    );
    assert_contains(
        sql,
        "UNION ALL SELECT \"Номенклатура\" AS \"Номенклатура\", MAX(\"Период\") AS \"Период\", SUM(\"Количество\") AS \"Количество\", 1 AS \"__level\", MIN(MIN(\"__rn\")) OVER (PARTITION BY \"Номенклатура\") AS \"__g1\", 0 AS \"__f1\", 0 AS \"__rn\" FROM \"__totals_rows\" GROUP BY \"Номенклатура\"",
    );
    assert_contains(
        sql,
        "UNION ALL SELECT \"Номенклатура\", \"Период\", \"Количество\", 2 AS \"__level\", MIN(\"__rn\") OVER (PARTITION BY \"Номенклатура\") AS \"__g1\", 1 AS \"__f1\", \"__rn\" AS \"__rn\" FROM \"__totals_rows\") AS \"__totals\" ORDER BY \"__g1\", \"__f1\", \"__rn\"",
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
    assert_contains(&mssql.sql, "ORDER BY [__g1], [__f1], [__rn]");
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
        "(SUM(\"Количество\") * 2) AS \"Количество\", 0 AS \"__level\", MIN(MIN(\"__rn\")) OVER (PARTITION BY \"Номенклатура\") AS \"__g1\", 0 AS \"__f1\", 0 AS \"__g2\", 0 AS \"__f2\", 0 AS \"__rn\" FROM \"__totals_rows\" GROUP BY \"Номенклатура\"",
    );
    assert_contains(
        &two.sql,
        "1 AS \"__level\", MIN(MIN(\"__rn\")) OVER (PARTITION BY \"Номенклатура\") AS \"__g1\", 1 AS \"__f1\", MIN(MIN(\"__rn\")) OVER (PARTITION BY \"Период\") AS \"__g2\", 0 AS \"__f2\", 0 AS \"__rn\" FROM \"__totals_rows\" GROUP BY \"Номенклатура\", \"Период\"",
    );
    assert_contains(
        &two.sql,
        "2 AS \"__level\", MIN(\"__rn\") OVER (PARTITION BY \"Номенклатура\") AS \"__g1\", 1 AS \"__f1\", MIN(\"__rn\") OVER (PARTITION BY \"Период\") AS \"__g2\", 1 AS \"__f2\", \"__rn\" AS \"__rn\" FROM \"__totals_rows\") AS \"__totals\" ORDER BY \"__g1\", \"__f1\", \"__g2\", \"__f2\", \"__rn\"",
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
        "ROW_NUMBER() OVER (ORDER BY \"__totals_source\".\"К\") AS \"__rn\" FROM ((SELECT",
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
            QueryDiagnosticKind::Syntax,
            "references one hierarchical catalog",
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

fn hierarchical_snapshot() -> MetadataSnapshot {
    with_live_tables(support::snapshot(), |tables| {
        tables[0].columns.push(open_sdbl::metadata::LiveColumn {
            name: "_parentidrref".to_owned(),
            data_type: "bytea".to_owned(),
        });
    })
}

#[test]
fn renders_hierarchy_totals_with_recursive_ctes() {
    let snapshot = hierarchical_snapshot();
    let query = "ВЫБРАТЬ Ссылка КАК Товар, Code КАК Код ИЗ Справочник.OpenSdblMetadataProbe
         УПОРЯДОЧИТЬ ПО Код
         ИТОГИ КОЛИЧЕСТВО(Код) ПО Товар ИЕРАРХИЯ;";
    let compiled = compile(&snapshot, PostgresBackend, query, true).unwrap();
    let sql = &compiled.sql;
    assert!(
        sql.starts_with("WITH RECURSIVE \"__totals_rows\" AS ("),
        "{sql}"
    );
    assert_contains(
        sql,
        "\"__totals_source\".\"Товар\" AS \"__hk\" FROM (SELECT",
    );
    assert_contains(
        sql,
        "\"__totals_ancestors\" AS (SELECT DISTINCT r.\"__hk\" AS \"__leaf\", \"__totals_catalog\".\"_parentidrref\" AS \"__node\", 1 AS \"__steps\" FROM \"__totals_rows\" r JOIN \"_reference53\" AS \"__totals_catalog\" ON \"__totals_catalog\".\"_idrref\" = r.\"__hk\" WHERE \"__totals_catalog\".\"_parentidrref\" <> decode('00000000000000000000000000000000', 'hex') UNION ALL SELECT h.\"__leaf\"",
    );
    assert_contains(
        sql,
        "\"__totals_depths\" AS (SELECT \"__leaf\", MAX(\"__steps\") AS \"__depth\"",
    );
    assert_contains(
        sql,
        "\"__totals_nodes\" AS (SELECT x.\"__node\", MIN(x.\"__rn\") AS \"__rank\", MIN(x.\"__depth\") AS \"__depth\"",
    );
    assert_contains(
        sql,
        "\"__totals_paths\" AS (SELECT n.\"__node\", LPAD(CAST(n.\"__rank\" AS text), 12, '0') AS \"__path\" FROM \"__totals_nodes\" n WHERE NOT EXISTS (SELECT 1 FROM \"__totals_nodes\" p WHERE p.\"__node\" = n.\"__parent\") UNION ALL SELECT c.\"__node\", p.\"__path\" || '/' || LPAD(CAST(c.\"__rank\" AS text), 12, '0') FROM \"__totals_paths\" p JOIN \"__totals_nodes\" c ON c.\"__parent\" = p.\"__node\")",
    );
    // Hierarchy rows aggregate every row beneath the ancestor.
    assert_contains(
        sql,
        "SELECT \"__totals_nodes\".\"__node\" AS \"Товар\", (COUNT(\"Код\"))::text AS \"Код\", 0 + \"__totals_nodes\".\"__depth\" AS \"__level\", \"__totals_paths\".\"__path\" AS \"__path\", 0 AS \"__f1\", 0 AS \"__rn\" FROM \"__totals_rows\" JOIN \"__totals_nodes\" ON \"__totals_nodes\".\"__hier\" = 1 AND (\"__totals_nodes\".\"__node\" = \"__hk\" OR EXISTS (SELECT 1 FROM \"__totals_ancestors\" a WHERE a.\"__leaf\" = \"__hk\" AND a.\"__node\" = \"__totals_nodes\".\"__node\")) JOIN \"__totals_paths\" ON \"__totals_paths\".\"__node\" = \"__totals_nodes\".\"__node\" GROUP BY \"__totals_nodes\".\"__node\", \"__totals_nodes\".\"__depth\", \"__totals_paths\".\"__path\"",
    );
    // Group rows key on the hierarchy key and sort after their folder.
    assert_contains(
        sql,
        "UNION ALL SELECT \"__hk\" AS \"Товар\", (COUNT(\"Код\"))::text AS \"Код\", 0 + (\"__totals_nodes\".\"__depth\" + \"__totals_nodes\".\"__hier\") AS \"__level\", \"__totals_paths\".\"__path\" AS \"__path\", 1 AS \"__f1\", 0 AS \"__rn\" FROM \"__totals_rows\" JOIN \"__totals_nodes\" ON \"__totals_nodes\".\"__node\" = \"__hk\" JOIN \"__totals_paths\" ON \"__totals_paths\".\"__node\" = \"__hk\" GROUP BY \"__hk\", \"__totals_nodes\".\"__depth\", \"__totals_nodes\".\"__hier\", \"__totals_paths\".\"__path\"",
    );
    assert_contains(
        sql,
        "1 + (\"__totals_nodes\".\"__depth\" + \"__totals_nodes\".\"__hier\") AS \"__level\", \"__totals_paths\".\"__path\" AS \"__path\", 2 AS \"__f1\", \"__rn\" AS \"__rn\" FROM \"__totals_rows\" JOIN \"__totals_nodes\"",
    );
    assert!(
        sql.ends_with("ORDER BY \"__path\", \"__f1\", \"__rn\""),
        "{sql}"
    );
    assert_eq!(compiled.columns[2].label, "__level");

    let mssql = compile(&snapshot, MsSqlBackend::new(0).unwrap(), query, false).unwrap();
    assert!(
        mssql.sql.starts_with("WITH [__totals_rows] AS ("),
        "{}",
        mssql.sql
    );
    assert_contains(
        &mssql.sql,
        "CAST(RIGHT('000000000000' + CAST(n.[__rank] AS varchar(12)), 12) AS varchar(4000)) AS [__path]",
    );
    assert_contains(
        &mssql.sql,
        "CAST(p.[__path] + '/' + RIGHT('000000000000' + CAST(c.[__rank] AS varchar(12)), 12) AS varchar(4000))",
    );

    let only = compile(
        &snapshot,
        PostgresBackend,
        "ВЫБРАТЬ Ссылка КАК Товар, Code КАК Код ИЗ Справочник.OpenSdblMetadataProbe
         ИТОГИ КОЛИЧЕСТВО(Код) ПО ОБЩИЕ, Товар ТОЛЬКО ИЕРАРХИЯ;",
        false,
    )
    .unwrap();
    assert_contains(
        &only.sql,
        "COALESCE(\"__totals_catalog\".\"_parentidrref\", decode('00000000000000000000000000000000', 'hex')) AS \"__hk\" FROM (SELECT",
    );
    assert_contains(
        &only.sql,
        ") AS \"__totals_source\" LEFT JOIN \"_reference53\" AS \"__totals_catalog\" ON \"__totals_catalog\".\"_idrref\" = \"__totals_source\".\"Товар\")",
    );
    assert_contains(
        &only.sql,
        "0 AS \"__level\", '' AS \"__path\", 0 AS \"__f1\", 0 AS \"__rn\" FROM \"__totals_rows\" HAVING COUNT(*) > 0",
    );
    assert_contains(
        &only.sql,
        "1 + \"__totals_nodes\".\"__depth\" AS \"__level\"",
    );

    let doubled = compile(
        &snapshot,
        PostgresBackend,
        "ВЫБРАТЬ Ссылка КАК Товар, Code КАК Код ИЗ Справочник.OpenSdblMetadataProbe
         ИТОГИ КОЛИЧЕСТВО(Код) ПО Товар, Товар ИЕРАРХИЯ;",
        false,
    )
    .unwrap();
    assert!(!doubled.sql.contains("\"__g2\""), "{}", doubled.sql);
    assert!(!doubled.sql.contains("\"__g1\""), "{}", doubled.sql);

    let twice = compile(
        &snapshot,
        PostgresBackend,
        "ВЫБРАТЬ Ссылка КАК Товар, Code КАК Код ИЗ Справочник.OpenSdblMetadataProbe
         ИТОГИ КОЛИЧЕСТВО(Код) ПО Товар ИЕРАРХИЯ, Код, Товар ИЕРАРХИЯ;",
        false,
    )
    .unwrap_err();
    assert_eq!(twice.kind(), QueryDiagnosticKind::UnsupportedFeature);
    assert!(twice.message().contains("only one hierarchical"));

    let batch = compile(
        &snapshot,
        PostgresBackend,
        "ВЫБРАТЬ Ссылка КАК Товар, Code КАК Код ПОМЕСТИТЬ ВТ ИЗ Справочник.OpenSdblMetadataProbe;
         ВЫБРАТЬ Х.Товар КАК Товар, Х.Код КАК Код ИЗ ВТ КАК Х ИТОГИ КОЛИЧЕСТВО(Код) ПО Товар ИЕРАРХИЯ;",
        false,
    )
    .unwrap();
    assert!(
        batch.sql.starts_with("WITH RECURSIVE \"vt1\" AS (SELECT"),
        "{}",
        batch.sql
    );
}
