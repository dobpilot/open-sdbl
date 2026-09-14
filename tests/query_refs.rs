//! The `ССЫЛКА`/`REFS` reference-type test.

mod support;

use support::*;

use open_sdbl::metadata::MetadataSnapshot;
use open_sdbl::query::{
    Backend, CompiledQuery, MsSqlBackend, PostgresBackend, QueryCompiler, QueryDiagnostic,
    QueryDiagnosticKind,
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

const DOCUMENT: &str = "Документ.бит_ДополнительныеУсловияПоДоговору";
const CATALOG: &str = "Справочник.ЦентрыФинансовойОтветственности";

#[test]
fn compares_the_type_member_of_a_composite_field() {
    let snapshot = universal_dereferenced_presentation_snapshot();
    let query = format!("ВЫБРАТЬ Ссылка ИЗ {DOCUMENT} ГДЕ ДоговорКонтрагента ССЫЛКА {CATALOG};");
    let compiled = postgres(&snapshot, &query);
    assert_contains(
        &compiled.sql,
        "WHERE (\"__src\".\"_fld59_rtref\" = decode('0000003e', 'hex'))",
    );
    let mssql = compile(&snapshot, MsSqlBackend::new(0).unwrap(), &query).unwrap();
    assert_contains(&mssql.sql, "WHERE ([__src].[_fld59_rtref] = 0x0000003e)");

    let negated = postgres(
        &snapshot,
        &format!("ВЫБРАТЬ Ссылка ИЗ {DOCUMENT} ГДЕ НЕ (ДоговорКонтрагента ССЫЛКА {CATALOG});"),
    );
    assert_contains(
        &negated.sql,
        "WHERE (NOT (\"__src\".\"_fld59_rtref\" = decode('0000003e', 'hex')))",
    );

    let case = postgres(
        &snapshot,
        &format!(
            "ВЫБРАТЬ ВЫБОР КОГДА ДоговорКонтрагента ССЫЛКА {CATALOG} ТОГДА 1 ИНАЧЕ 0 КОНЕЦ КАК Р ИЗ {DOCUMENT};"
        ),
    );
    assert_contains(
        &case.sql,
        "CASE WHEN (\"__src\".\"_fld59_rtref\" = decode('0000003e', 'hex')) THEN 1 ELSE 0 END AS \"Р\"",
    );
}

#[test]
fn tests_the_payload_prefix_of_a_derived_column() {
    let snapshot = universal_dereferenced_presentation_snapshot();
    let compiled = postgres(
        &snapshot,
        &format!(
            "ВЫБРАТЬ Т.Д ИЗ (ВЫБРАТЬ ДоговорКонтрагента КАК Д ИЗ {DOCUMENT}) КАК Т ГДЕ Т.Д ССЫЛКА {CATALOG};"
        ),
    );
    assert_contains(
        &compiled.sql,
        "WHERE (substring(\"Т\".\"Д\" from 1 for 4) = decode('0000003e', 'hex'))",
    );
}

#[test]
fn fixed_target_fields_are_always_of_their_type() {
    let snapshot = tabular_section_snapshot();
    let compiled = postgres(
        &snapshot,
        &format!("ВЫБРАТЬ Ссылка ИЗ {DOCUMENT} ГДЕ ДоговорКонтрагента ССЫЛКА {CATALOG};"),
    );
    assert_contains(&compiled.sql, "WHERE TRUE");
    let mssql = compile(
        &snapshot,
        MsSqlBackend::new(0).unwrap(),
        &format!("SELECT Ссылка FROM {DOCUMENT} WHERE ДоговорКонтрагента REFS {CATALOG};"),
    )
    .unwrap();
    assert_contains(&mssql.sql, "WHERE (1 = 1)");

    let own = postgres(
        &snapshot,
        &format!("ВЫБРАТЬ Ссылка ИЗ {DOCUMENT} ГДЕ Ссылка ССЫЛКА {DOCUMENT};"),
    );
    assert_eq!(own.columns[0].label, "ID");
    assert_contains(&own.sql, "WHERE TRUE");
}

#[test]
fn reports_incompatible_operands() {
    let snapshot = tabular_section_snapshot();
    for (query, kind, message) in [
        (
            format!("ВЫБРАТЬ Ссылка ИЗ {DOCUMENT} ГДЕ ДоговорКонтрагента ССЫЛКА {DOCUMENT};"),
            QueryDiagnosticKind::Syntax,
            "cannot hold",
        ),
        (
            format!("ВЫБРАТЬ Ссылка ИЗ {DOCUMENT} ГДЕ Ссылка ССЫЛКА Справочник.Несуществующий;"),
            QueryDiagnosticKind::UnknownObject,
            "could not be resolved",
        ),
        (
            format!("ВЫБРАТЬ Ссылка ИЗ {DOCUMENT} ГДЕ 1 ССЫЛКА {CATALOG};"),
            QueryDiagnosticKind::Syntax,
            "must be a reference field",
        ),
        (
            "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE Code REFS Catalog.OpenSdblMetadataProbe;"
                .to_owned(),
            QueryDiagnosticKind::Syntax,
            "must be a reference field",
        ),
        (
            format!("ВЫБРАТЬ 1 КАК А ГДЕ 1 ССЫЛКА {CATALOG};"),
            QueryDiagnosticKind::UnsupportedFeature,
            "requires FROM",
        ),
    ] {
        let snapshot = if query.contains("OpenSdblMetadataProbe") {
            support::snapshot()
        } else {
            snapshot.clone()
        };
        let error = compile(&snapshot, PostgresBackend, &query).unwrap_err();
        assert_eq!(error.kind(), kind, "{query}: {error}");
        assert!(error.message().contains(message), "{query}: {error}");
    }
}

#[test]
fn compares_a_reference_with_a_composite_subquery() {
    // A subquery whose value is composite projects one column per member.
    // Measured on 8.3.27: the platform compares the members side by side,
    // spreading the other side over the same members — its own carries the
    // value, the discriminator its tag, the rest the zero of their type,
    // and all of them stay NULL while the value is NULL.
    let mut session = open_sdbl::query::SessionParameters::new();
    for name in ["ЗначениеРазделителя", "ОбластьДанныхОсновныеДанные"]
    {
        session.set(open_sdbl::query::QueryParameter::new(
            name,
            open_sdbl::query::ParameterValue::Number {
                unscaled: 0,
                scale: 0,
            },
        ));
    }
    session.set(open_sdbl::query::QueryParameter::new(
        "ИспользованиеРазделителя",
        open_sdbl::query::ParameterValue::Boolean(false),
    ));
    let snapshot = support::demo_resolved_at(
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/demo"),
    )
    .snapshot;
    let source = "ВЫБРАТЬ С.Ссылка КАК С ИЗ Справочник.Сотрудники КАК С
         ГДЕ С.Ссылка В (ВЫБРАТЬ П.Руководитель ИЗ Справочник.Проекты КАК П);";
    let compiled = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile_with(
            source,
            &open_sdbl::query::CompileOptions::new().session(&session),
        )
        .expect("a composite subquery compiles on PostgreSQL");
    assert!(
        compiled
            .sql
            .contains("IS NOT NULL THEN decode('08', 'hex') END, ("),
        "the reference is spread over the members of the subquery: {}",
        compiled.sql
    );
    assert!(
        compiled.sql.contains(") IN (SELECT "),
        "the members are compared side by side: {}",
        compiled.sql
    );

    // T-SQL has no row comparison, so the same query is refused there.
    let mssql = QueryCompiler::new(&snapshot, MsSqlBackend::new(0).unwrap())
        .compile_with(
            source,
            &open_sdbl::query::CompileOptions::new().session(&session),
        )
        .unwrap_err();
    assert_eq!(mssql.kind(), QueryDiagnosticKind::UnsupportedFeature);
}
