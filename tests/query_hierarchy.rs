//! `В ИЕРАРХИИ`: membership in the subtree of the seed references.

mod support;

use support::*;

use open_sdbl::metadata::MetadataSnapshot;
use open_sdbl::query::{
    Backend, CompiledQuery, MsSqlBackend, PostgresBackend, QueryCompiler, QueryDiagnostic,
    QueryDiagnosticKind, TempTablesManager,
};

fn compile<B: Backend>(
    snapshot: &MetadataSnapshot,
    backend: B,
    source: &str,
) -> Result<CompiledQuery, QueryDiagnostic> {
    QueryCompiler::new(snapshot, backend).compile(source)
}

fn postgres(snapshot: &MetadataSnapshot, source: &str) -> CompiledQuery {
    compile(snapshot, PostgresBackend, source).unwrap_or_else(|error| panic!("{source}: {error}"))
}

fn assert_contains(sql: &str, needle: &str) {
    assert!(sql.contains(needle), "expected {needle:?} in\n{sql}");
}

/// The probe catalog of the hierarchy tests carries a live parent column.
fn hierarchical_snapshot() -> MetadataSnapshot {
    with_live_tables(support::snapshot(), |tables| {
        tables[0].columns.push(open_sdbl::metadata::LiveColumn {
            name: "_parentidrref".to_owned(),
            data_type: "bytea".to_owned(),
        });
    })
}

const CATALOG: &str = "Справочник.OpenSdblMetadataProbe";

#[test]
fn descends_from_the_seeds_of_a_nested_query() {
    let snapshot = hierarchical_snapshot();
    let query = format!(
        "ВЫБРАТЬ Код ИЗ {CATALOG}
         ГДЕ Ссылка В ИЕРАРХИИ (ВЫБРАТЬ Г.Ссылка ИЗ {CATALOG} КАК Г ГДЕ Г.Код = \"A\");"
    );
    let compiled = postgres(&snapshot, &query);
    assert!(
        compiled.sql.starts_with("WITH RECURSIVE \"__hier_1\" AS ("),
        "{}",
        compiled.sql
    );
    // The seeds come from the nested query, and the recursive term walks
    // down the parent column of the target catalog.
    assert_contains(
        &compiled.sql,
        "SELECT \"__seeds\".\"Ссылка\" AS \"__node\" FROM (SELECT \"Г\".\"_idrref\" AS \"Ссылка\" FROM \"_reference53\" AS \"Г\" WHERE (\"Г\".\"_code\" = 'A')) AS \"__seeds\" UNION ALL SELECT \"__catalog\".\"_idrref\" FROM \"_reference53\" AS \"__catalog\" JOIN \"__hier_1\" ON \"__catalog\".\"_parentidrref\" = \"__hier_1\".\"__node\"",
    );
    assert_contains(
        &compiled.sql,
        "WHERE EXISTS (SELECT 1 FROM \"__hier_1\" WHERE \"__node\" = \"__src\".\"_idrref\")",
    );

    // SQL Server spells a recursive CTE with a plain WITH.
    let mssql = compile(&snapshot, MsSqlBackend::new(0).unwrap(), &query).unwrap();
    assert!(
        mssql.sql.starts_with("WITH [__hier_1] AS ("),
        "{}",
        mssql.sql
    );
    assert_contains(
        &mssql.sql,
        "JOIN [__hier_1] ON [__catalog].[_parentidrref] = [__hier_1].[__node]",
    );
}

#[test]
fn negates_and_accepts_constant_seeds() {
    let snapshot = hierarchical_snapshot();
    let negated = postgres(
        &snapshot,
        &format!(
            "ВЫБРАТЬ Код ИЗ {CATALOG} ГДЕ Ссылка НЕ В ИЕРАРХИИ (0x{});",
            "11".repeat(16)
        ),
    );
    assert_contains(
        &negated.sql,
        "WHERE NOT EXISTS (SELECT 1 FROM \"__hier_1\" WHERE \"__node\" = \"__src\".\"_idrref\")",
    );
    assert_contains(
        &negated.sql,
        &format!(
            "SELECT '\\x{}'::bytea AS \"__node\" UNION ALL",
            "11".repeat(16)
        ),
    );

    // Two predicates get one CTE each.
    let two = postgres(
        &snapshot,
        &format!(
            "ВЫБРАТЬ Код ИЗ {CATALOG}
             ГДЕ Ссылка В ИЕРАРХИИ (0x{a}) ИЛИ Ссылка В ИЕРАРХИИ (0x{b});",
            a = "11".repeat(16),
            b = "22".repeat(16)
        ),
    );
    assert!(
        two.sql.starts_with("WITH RECURSIVE \"__hier_1\" AS ("),
        "{}",
        two.sql
    );
    assert_contains(&two.sql, ", \"__hier_2\" AS (");
}

#[test]
fn falls_back_to_membership_without_a_parent_column() {
    // The plain probe catalog has no `_ParentIDRRef`, so there is no
    // hierarchy to descend and the predicate is plain membership.
    let snapshot = support::snapshot();
    let compiled = postgres(
        &snapshot,
        &format!(
            "ВЫБРАТЬ Код ИЗ {CATALOG} ГДЕ Ссылка В ИЕРАРХИИ (0x{});",
            "11".repeat(16)
        ),
    );
    assert!(!compiled.sql.contains("WITH"), "{}", compiled.sql);
    assert_contains(
        &compiled.sql,
        "WHERE EXISTS (SELECT 1 FROM (SELECT '\\x11111111111111111111111111111111'::bytea AS \"__node\") AS \"__seeds_flat\" WHERE \"__node\" = \"__src\".\"_idrref\")",
    );
}

#[test]
fn reports_unsupported_operands() {
    let snapshot = hierarchical_snapshot();
    for (query, message) in [
        (
            format!(
                "ВЫБРАТЬ Код ИЗ {CATALOG} ГДЕ Код В ИЕРАРХИИ (0x{});",
                "11".repeat(16)
            ),
            "to reference one catalog",
        ),
        (
            format!("ВЫБРАТЬ Код ИЗ {CATALOG} ГДЕ Ссылка В ИЕРАРХИИ (Ссылка);"),
            "not a field",
        ),
        (
            format!("ВЫБРАТЬ 1 КАК А ГДЕ 1 В ИЕРАРХИИ (0x{});", "11".repeat(16)),
            "requires FROM",
        ),
    ] {
        let error = compile(&snapshot, PostgresBackend, &query).unwrap_err();
        assert_eq!(
            error.kind(),
            QueryDiagnosticKind::UnsupportedFeature,
            "{query}: {error}"
        );
        assert!(error.message().contains(message), "{query}: {error}");
    }

    // A temporary-table body becomes a CTE, and SQL Server forbids a
    // nested WITH there: the hierarchy CTE moves beside the table's own,
    // named by the table, and leads the WITH list of every reader.
    let batch = format!(
        "ВЫБРАТЬ Код КАК Код ПОМЕСТИТЬ ВТ ИЗ {CATALOG} ГДЕ Ссылка В ИЕРАРХИИ (0x{});
         ВЫБРАТЬ Код ИЗ ВТ;",
        "11".repeat(16)
    );
    let options = open_sdbl::query::CompileOptions::new();
    let mut manager = TempTablesManager::new();
    let postgres_batch = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile_batch(&batch, &options, &mut manager)
        .unwrap()
        .unwrap()
        .sql;
    for needle in [
        "WITH RECURSIVE \"__hier_1_t1\" AS (",
        ", \"vt1\" AS (SELECT",
        "FROM \"__hier_1_t1\" WHERE",
    ] {
        assert_contains(&postgres_batch, needle);
    }
    assert!(!postgres_batch.contains("\"__hier_1\""), "{postgres_batch}");
    let mut manager = TempTablesManager::new();
    let mssql_batch = QueryCompiler::new(&snapshot, MsSqlBackend::new(2000).unwrap())
        .compile_batch(&batch, &options, &mut manager)
        .unwrap()
        .unwrap()
        .sql;
    for needle in [
        "WITH [__hier_1_t1] AS (",
        ", [vt1] AS (SELECT",
        "JOIN [__hier_1_t1] ON",
    ] {
        assert_contains(&mssql_batch, needle);
    }
}
