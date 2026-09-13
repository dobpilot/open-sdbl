//! Reference paths of more than one hop.

mod support;

use support::*;

use open_sdbl::metadata::MetadataSnapshot;
use open_sdbl::query::{
    CompiledQuery, MsSqlBackend, PostgresBackend, QueryCompiler, QueryDiagnosticKind,
};

fn postgres(snapshot: &MetadataSnapshot, source: &str) -> CompiledQuery {
    QueryCompiler::new(snapshot, PostgresBackend)
        .compile(source)
        .unwrap_or_else(|error| panic!("{source}: {error}"))
}

fn assert_contains(sql: &str, needle: &str) {
    assert!(sql.contains(needle), "expected {needle:?} in\n{sql}");
}

const CATALOG: &str = "Справочник.OpenSdblMetadataProbe";

#[test]
fn walks_a_reference_chain() {
    let snapshot = chained_reference_snapshot();
    let two = postgres(
        &snapshot,
        &format!("ВЫБРАТЬ Т.Организация.Организация.Код КАК К ИЗ {CATALOG} КАК Т;"),
    );
    // Each hop joins the target of the previous one.
    assert_contains(
        &two.sql,
        "LEFT JOIN \"_reference57\" AS \"__ref1\" ON \"Т\".\"_fld54\" = \"__ref1\".\"_idrref\"",
    );
    assert_contains(
        &two.sql,
        "LEFT JOIN \"_reference57\" AS \"__ref2\" ON \"__ref1\".\"_fld54\" = \"__ref2\".\"_idrref\"",
    );
    assert_contains(&two.sql, "\"__ref2\".\"_code\"::text AS \"К\"");

    let three = postgres(
        &snapshot,
        &format!("ВЫБРАТЬ Т.Организация.Организация.Организация.Код КАК К ИЗ {CATALOG} КАК Т;"),
    );
    assert_contains(
        &three.sql,
        "AS \"__ref3\" ON \"__ref2\".\"_fld54\" = \"__ref3\".\"_idrref\"",
    );
}

#[test]
fn shares_the_joins_of_a_repeated_prefix() {
    let snapshot = chained_reference_snapshot();
    let compiled = postgres(
        &snapshot,
        &format!(
            "ВЫБРАТЬ Т.Организация.Организация.Код КАК К, Т.Организация.Организация.Дата КАК Д,
                    Т.Организация.Код КАК К2 ИЗ {CATALOG} КАК Т;"
        ),
    );
    assert_eq!(
        compiled.sql.matches("LEFT JOIN").count(),
        2,
        "{}",
        compiled.sql
    );
}

#[test]
fn walks_the_chain_in_every_clause() {
    let snapshot = chained_reference_snapshot();
    let filtered = postgres(
        &snapshot,
        &format!(
            "ВЫБРАТЬ Т.Код КАК К ИЗ {CATALOG} КАК Т
             ГДЕ Т.Организация.Организация.Код = \"A\"
             УПОРЯДОЧИТЬ ПО Т.Организация.Организация.Дата;"
        ),
    );
    assert_contains(&filtered.sql, "WHERE (\"__ref2\".\"_code\" = 'A')");
    assert_contains(&filtered.sql, "ORDER BY \"__ref2\".\"_date_time\" ASC");

    let grouped = postgres(
        &snapshot,
        &format!(
            "ВЫБРАТЬ Т.Организация.Организация.Код КАК К, КОЛИЧЕСТВО(*) КАК Ч ИЗ {CATALOG} КАК Т
             СГРУППИРОВАТЬ ПО Т.Организация.Организация.Код;"
        ),
    );
    assert_contains(&grouped.sql, "GROUP BY \"__ref2\".\"_code\"");

    let mssql = QueryCompiler::new(&snapshot, MsSqlBackend::new(0).unwrap())
        .compile(&format!(
            "ВЫБРАТЬ Т.Организация.Организация.Код КАК К ИЗ {CATALOG} КАК Т;"
        ))
        .unwrap();
    assert_contains(
        &mssql.sql,
        "LEFT JOIN [_reference57] AS [__ref2] ON [__ref1].[_fld54] = [__ref2].[_idrref]",
    );
}

#[test]
fn refuses_to_walk_through_a_composite_reference() {
    let snapshot = universal_dereferenced_presentation_snapshot();
    let error = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile(
            "ВЫБРАТЬ ДоговорКонтрагента.Организация.Код КАК К
             ИЗ Документ.бит_ДополнительныеУсловияПоДоговору;",
        )
        .unwrap_err();
    // The walk cannot continue through a value selected by type.
    assert!(
        matches!(
            error.kind(),
            QueryDiagnosticKind::UnsupportedFeature | QueryDiagnosticKind::UnknownObject
        ),
        "{error}"
    );
}
