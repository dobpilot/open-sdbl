//! Register virtual tables: what they group by, and their bare form.

mod support;

use support::*;

use open_sdbl::metadata::MetadataSnapshot;
use open_sdbl::query::{CompiledQuery, MsSqlBackend, PostgresBackend, QueryCompiler};

fn postgres(snapshot: &MetadataSnapshot, source: &str) -> CompiledQuery {
    QueryCompiler::new(snapshot, PostgresBackend)
        .compile(source)
        .unwrap_or_else(|error| panic!("{source}: {error}"))
}

fn assert_contains(sql: &str, needle: &str) {
    assert!(sql.contains(needle), "expected {needle:?} in\n{sql}");
}

#[test]
fn sums_over_the_dimensions_the_statement_never_reads() {
    let snapshot = accumulation_register_snapshot();
    // The platform answers one row holding the turnover of the register
    // when the query reads no dimension.
    let resource_only = postgres(
        &snapshot,
        "ВЫБРАТЬ О.КоличествоОборот КАК Кол ИЗ РегистрНакопления.Остатки.Обороты КАК О;",
    );
    assert_contains(
        &resource_only.sql,
        "FROM (SELECT SUM(\"__aggregate_used\".\"_fld55\") AS \"_fld55\" FROM (SELECT",
    );
    assert!(
        !resource_only.sql.contains("GROUP BY \"__aggregate_used\""),
        "{}",
        resource_only.sql
    );

    // Reading the dimension keeps the relation as it was.
    let with_dimension = postgres(
        &snapshot,
        "ВЫБРАТЬ О.Номенклатура КАК Н, О.КоличествоОборот КАК Кол
         ИЗ РегистрНакопления.Остатки.Обороты КАК О;",
    );
    assert!(
        !with_dimension.sql.contains("__aggregate_used"),
        "{}",
        with_dimension.sql
    );
    assert_contains(
        &with_dimension.sql,
        "GROUP BY \"__aggregate_base\".\"_fld54\"",
    );

    // Balances aggregate the same way.
    let balance = postgres(
        &snapshot,
        "ВЫБРАТЬ О.КоличествоОстаток КАК Кол ИЗ РегистрНакопления.Остатки.Остатки КАК О;",
    );
    assert_contains(&balance.sql, "\"__aggregate_used\"");
    assert_contains(&balance.sql, "SUM(\"__aggregate_used\".\"_fld55\")");
}

#[test]
fn accepts_virtual_tables_without_an_argument_list() {
    let snapshot = accumulation_register_snapshot();
    let bare = postgres(
        &snapshot,
        "ВЫБРАТЬ О.Номенклатура КАК Н ИЗ РегистрНакопления.Остатки.Остатки КАК О;",
    );
    let parenthesized = postgres(
        &snapshot,
        "ВЫБРАТЬ О.Номенклатура КАК Н ИЗ РегистрНакопления.Остатки.Остатки() КАК О;",
    );
    assert_eq!(bare.sql, parenthesized.sql);

    let mssql = QueryCompiler::new(&snapshot, MsSqlBackend::new(0).unwrap())
        .compile("ВЫБРАТЬ О.Номенклатура КАК Н ИЗ РегистрНакопления.Остатки.Обороты КАК О;")
        .unwrap();
    assert_contains(&mssql.sql, "GROUP BY [__aggregate_base].[_fld54]");
}
