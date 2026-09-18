//! `(А, Б) В (ВЫБРАТЬ …)`: a tuple tested against a subquery.

mod support;

use std::path::PathBuf;

use open_sdbl::metadata::MetadataSnapshot;
use open_sdbl::query::{
    CompileOptions, MsSqlBackend, ParameterValue, PostgresBackend, QueryCompiler,
    QueryDiagnosticKind, QueryParameter, SessionParameters,
};
use support::*;

fn postgres(snapshot: &MetadataSnapshot, source: &str) -> String {
    QueryCompiler::new(snapshot, PostgresBackend)
        .compile(source)
        .unwrap_or_else(|error| panic!("{source}: {error}"))
        .sql
}

fn assert_contains(sql: &str, needle: &str) {
    assert!(sql.contains(needle), "expected {needle:?} in\n{sql}");
}

#[test]
fn a_tuple_membership_test_renders_as_exists() {
    let snapshot = snapshot();
    let sql = postgres(
        &snapshot,
        "ВЫБРАТЬ Т.Код КАК Код ИЗ Справочник.OpenSdblMetadataProbe КАК Т
         ГДЕ (Т.Код, Т.ProbeAttribute) В (ВЫБРАТЬ П.Код, П.ProbeAttribute ИЗ Справочник.OpenSdblMetadataProbe КАК П ГДЕ П.Код = \"A\");",
    );
    assert_contains(
        &sql,
        "WHERE EXISTS (SELECT 1 FROM (SELECT \"П\".\"_code\"::text AS \"Код\", \"П\".\"_fld54\" AS \"ProbeAttribute\" FROM \"_reference53\" AS \"П\" WHERE (\"П\".\"_code\" = 'A')) AS \"__in\" WHERE \"__in\".\"Код\" = (\"Т\".\"_code\")::text AND \"__in\".\"ProbeAttribute\" = \"Т\".\"_fld54\")",
    );
    let negated = postgres(
        &snapshot,
        "ВЫБРАТЬ Т.Код КАК Код ИЗ Справочник.OpenSdblMetadataProbe КАК Т
         ГДЕ (Т.Код, Т.ProbeAttribute) НЕ В (ВЫБРАТЬ П.Код, П.ProbeAttribute ИЗ Справочник.OpenSdblMetadataProbe КАК П);",
    );
    assert_contains(&negated, "WHERE (NOT EXISTS (SELECT 1 FROM (");
    let mssql = QueryCompiler::new(&snapshot, MsSqlBackend::new(2000).unwrap())
        .compile(
            "ВЫБРАТЬ Т.Код КАК Код ИЗ Справочник.OpenSdblMetadataProbe КАК Т
             ГДЕ (Т.Код, Т.ProbeAttribute) В (ВЫБРАТЬ П.Код, П.ProbeAttribute ИЗ Справочник.OpenSdblMetadataProbe КАК П);",
        )
        .unwrap()
        .sql;
    assert_contains(&mssql, "WHERE EXISTS (SELECT 1 FROM (SELECT");
    assert_contains(
        &mssql,
        "AS [__in] WHERE [__in].[Код] = CONVERT(nvarchar(max), [Т].[_code]) AND [__in].[ProbeAttribute] = [Т].[_fld54])",
    );
}

#[test]
fn a_tuple_needs_a_matching_subquery() {
    let snapshot = snapshot();
    let compiler = QueryCompiler::new(&snapshot, PostgresBackend);
    let mismatch = compiler
        .compile(
            "ВЫБРАТЬ Т.Код КАК Код ИЗ Справочник.OpenSdblMetadataProbe КАК Т
             ГДЕ (Т.Код, Т.ProbeAttribute) В (ВЫБРАТЬ П.Код ИЗ Справочник.OpenSdblMetadataProbe КАК П);",
        )
        .unwrap_err();
    assert_eq!(mismatch.kind(), QueryDiagnosticKind::UnsupportedFeature);
    assert!(
        mismatch.message().contains("2 columns"),
        "{}",
        mismatch.message()
    );
    let elsewhere = compiler
        .compile(
            "ВЫБРАТЬ (Т.Код, Т.ProbeAttribute) КАК Пара ИЗ Справочник.OpenSdblMetadataProbe КАК Т;",
        )
        .unwrap_err();
    assert_eq!(elsewhere.kind(), QueryDiagnosticKind::UnsupportedFeature);
    assert!(
        elsewhere.message().contains("tuple"),
        "{}",
        elsewhere.message()
    );
}

/// The UNF fixture carries registers with two dimensions to test a pair
/// in a virtual-table condition.
fn unf() -> Option<MetadataSnapshot> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/unf");
    root.join("db_names.deflate")
        .is_file()
        .then(|| demo_resolved_at(&root).snapshot)
}

#[test]
fn a_tuple_is_accepted_in_a_virtual_table_condition() {
    let Some(snapshot) = unf() else {
        return;
    };
    let mut session = SessionParameters::new();
    let zero = ParameterValue::Number {
        unscaled: 0,
        scale: 0,
    };
    for name in ["ОбластьДанныхЗначение", "ОбластьДанныхОсновныеДанные"]
    {
        session.set(QueryParameter::new(name, zero.clone()));
    }
    session.set(QueryParameter::new(
        "ОбластьДанныхИспользование",
        ParameterValue::Boolean(false),
    ));
    let parameters = [QueryParameter::new("Заказ", ParameterValue::Null)];
    let options = CompileOptions::new()
        .session(&session)
        .parameters(&parameters);
    let source = "ВЫБРАТЬ О.Номенклатура КАК Н, О.КоличествоОстаток КАК К
         ИЗ РегистрНакопления.ЗапасыНаСкладах.Остатки(, (Номенклатура, Характеристика) В (ВЫБРАТЬ С.Номенклатура, С.Характеристика ИЗ Документ.ЗаказПокупателя.Запасы КАК С ГДЕ С.Ссылка = &Заказ)) КАК О;";
    let sql = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile_with(source, &options)
        .unwrap_or_else(|error| panic!("{source}: {error}"))
        .sql;
    assert_contains(&sql, "EXISTS (SELECT 1 FROM (SELECT");
    // The tabular section's Номенклатура is composite (a string or a
    // reference), so the register's reference spreads over its members.
    assert_contains(&sql, "\"__in\".\"Номенклатура_TYPE\" = ");
    assert_contains(&sql, "\"__in\".\"Номенклатура_RRRef\" = \"__totals_base\"");
    assert_contains(&sql, "\"__in\".\"Характеристика\" = ");
}

#[test]
fn a_string_membership_test_compares_text_with_text() {
    let snapshot = snapshot();
    let sql = postgres(
        &snapshot,
        "ВЫБРАТЬ Т.Код КАК Код ИЗ Справочник.OpenSdblMetadataProbe КАК Т
         ГДЕ Т.Код В (ВЫБРАТЬ П.Код ИЗ Справочник.OpenSdblMetadataProbe КАК П);",
    );
    // PostgreSQL has no operator between mvarchar and the text the
    // subquery projects, so the outer side is cast as well.
    assert_contains(
        &sql,
        "((\"Т\".\"_code\")::text IN (SELECT \"П\".\"_code\"::text AS \"Код\"",
    );
}
