//! `ТИП`, `ТИПЗНАЧЕНИЯ`, and the `НЕОПРЕДЕЛЕНО` literal.

mod support;

use support::*;

use open_sdbl::metadata::{MetadataKind, MetadataSnapshot};
use open_sdbl::query::{
    Backend, ColumnKind, CompileOptions, CompiledQuery, MsSqlBackend, ParameterValue,
    PostgresBackend, QueryCompiler, QueryDiagnostic, QueryDiagnosticKind, QueryParameter,
    TypeValue,
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

fn mssql(snapshot: &MetadataSnapshot, source: &str) -> CompiledQuery {
    compile(snapshot, MsSqlBackend::new(0).unwrap(), source)
        .unwrap_or_else(|error| panic!("{source}: {error}"))
}

fn assert_contains(sql: &str, needle: &str) {
    assert!(sql.contains(needle), "expected {needle:?} in\n{sql}");
}

const DOCUMENT: &str = "Документ.бит_ДополнительныеУсловияПоДоговору";
const CATALOG: &str = "Справочник.ЦентрыФинансовойОтветственности";
/// The type of a composite value, as the platform stores it: the `_TYPE`
/// tag, and the table number when the tag says the value is a reference.
const COMPOSITE_TYPE: &str = "COALESCE(CASE WHEN \"__src\".\"_fld59_type\" = decode('08', 'hex') THEN (\"__src\".\"_fld59_type\" || \"__src\".\"_fld59_rtref\") ELSE (\"__src\".\"_fld59_type\" || decode('00000000', 'hex')) END, decode('0000000000', 'hex'))";

#[test]
fn encodes_the_type_of_every_value() {
    let snapshot = universal_dereferenced_presentation_snapshot();
    let query = format!(
        "ВЫБРАТЬ ТИПЗНАЧЕНИЯ(ДоговорКонтрагента) КАК Т, ТИПЗНАЧЕНИЯ(Ссылка) КАК С,
         ТИПЗНАЧЕНИЯ(1) КАК Ч, ТИПЗНАЧЕНИЯ(NULL) КАК Н, ТИПЗНАЧЕНИЯ(НЕОПРЕДЕЛЕНО) КАК О
         ИЗ {DOCUMENT};"
    );
    let compiled = postgres(&snapshot, &query);
    assert_contains(&compiled.sql, &format!("{COMPOSITE_TYPE} AS \"Т\""));
    // A fixed-target reference keeps its type even when the value is the
    // empty reference; only a missing row has the NULL type.
    assert_contains(
        &compiled.sql,
        "CASE WHEN \"__src\".\"_idrref\" IS NULL THEN decode('0000000000', 'hex') ELSE decode('0800000035', 'hex') END AS \"С\"",
    );
    assert_contains(&compiled.sql, "decode('0300000000', 'hex') AS \"Ч\"");
    assert_contains(&compiled.sql, "decode('0000000000', 'hex') AS \"Н\"");
    assert_contains(&compiled.sql, "decode('0100000000', 'hex') AS \"О\"");
    for column in &compiled.columns {
        assert_eq!(column.kind, ColumnKind::Type, "{}", column.label);
    }

    let mssql = mssql(&snapshot, &query);
    assert_contains(
        &mssql.sql,
        "COALESCE(CASE WHEN [__src].[_fld59_type] = 0x08 THEN ([__src].[_fld59_type] + [__src].[_fld59_rtref]) ELSE ([__src].[_fld59_type] + 0x00000000) END, 0x0000000000) AS [Т]",
    );
    assert_contains(&mssql.sql, "0x0300000000 AS [Ч]");
}

#[test]
fn compiles_type_literals() {
    let snapshot = universal_dereferenced_presentation_snapshot();
    let compiled = postgres(
        &snapshot,
        &format!(
            "ВЫБРАТЬ ТИП(Строка) КАК С, ТИП(Число) КАК Ч, ТИП(Дата) КАК Д, ТИП(Булево) КАК Б,
             ТИП({CATALOG}) КАК О ИЗ {DOCUMENT};"
        ),
    );
    for (label, bytes) in [
        ("С", "0500000000"),
        ("Ч", "0300000000"),
        ("Д", "0400000000"),
        ("Б", "0200000000"),
        ("О", "080000003e"),
    ] {
        assert_contains(
            &compiled.sql,
            &format!("decode('{bytes}', 'hex') AS \"{label}\""),
        );
    }
    // English spellings, and a statement without a source.
    let free = postgres(&snapshot, "SELECT TYPE(String) AS С, VALUETYPE(TRUE) AS Б;");
    assert_contains(&free.sql, "decode('0500000000', 'hex') AS \"С\"");
    assert_contains(&free.sql, "decode('0200000000', 'hex') AS \"Б\"");
    assert_eq!(free.columns[0].kind, ColumnKind::Type);
}

#[test]
fn compares_value_types_with_type_literals() {
    let snapshot = universal_dereferenced_presentation_snapshot();
    let compiled = postgres(
        &snapshot,
        &format!(
            "ВЫБРАТЬ Ссылка ИЗ {DOCUMENT} ГДЕ ТИПЗНАЧЕНИЯ(ДоговорКонтрагента) = ТИП({CATALOG});"
        ),
    );
    assert_contains(
        &compiled.sql,
        &format!("WHERE ({COMPOSITE_TYPE} = decode('080000003e', 'hex'))"),
    );

    let unequal = postgres(
        &snapshot,
        &format!(
            "ВЫБРАТЬ Ссылка ИЗ {DOCUMENT} ГДЕ ТИПЗНАЧЕНИЯ(ДоговорКонтрагента) <> ТИП(Строка);"
        ),
    );
    assert_contains(&unequal.sql, "<> decode('0500000000', 'hex'))");

    let list = postgres(
        &snapshot,
        &format!(
            "ВЫБРАТЬ Ссылка ИЗ {DOCUMENT}
             ГДЕ ТИПЗНАЧЕНИЯ(ДоговорКонтрагента) В (ТИП(Строка), ТИП({CATALOG}));"
        ),
    );
    assert_contains(
        &list.sql,
        "IN (decode('0500000000', 'hex'), decode('080000003e', 'hex')))",
    );
}

#[test]
fn reads_the_type_member_of_a_derived_column() {
    let snapshot = universal_dereferenced_presentation_snapshot();
    let compiled = postgres(
        &snapshot,
        &format!(
            "ВЫБРАТЬ ТИПЗНАЧЕНИЯ(Т.Д) КАК Т ИЗ (ВЫБРАТЬ ДоговорКонтрагента КАК Д ИЗ {DOCUMENT}) КАК Т;"
        ),
    );
    assert_contains(
        &compiled.sql,
        "COALESCE(CASE WHEN \"Т\".\"Д_TYPE\" = decode('08', 'hex') THEN (\"Т\".\"Д_TYPE\" || substring(\"Т\".\"Д\" from 1 for 4)) ELSE (\"Т\".\"Д_TYPE\" || decode('00000000', 'hex')) END, decode('0000000000', 'hex')) AS \"Т\"",
    );
    assert_contains(&compiled.sql, "\"__src\".\"_fld59_type\" AS \"Д_TYPE\"");
}

#[test]
fn compiles_the_undefined_literal() {
    let snapshot = universal_dereferenced_presentation_snapshot();
    let compiled = postgres(
        &snapshot,
        &format!("ВЫБРАТЬ НЕОПРЕДЕЛЕНО КАК Н ИЗ {DOCUMENT} ГДЕ ДоговорКонтрагента = НЕОПРЕДЕЛЕНО;"),
    );
    assert_contains(&compiled.sql, "SELECT NULL AS \"Н\"");
    assert_eq!(compiled.columns[0].kind, ColumnKind::Undefined);
    assert_contains(
        &compiled.sql,
        &format!("WHERE ({COMPOSITE_TYPE} = decode('0100000000', 'hex'))"),
    );

    let unequal = postgres(
        &snapshot,
        &format!("SELECT Ссылка FROM {DOCUMENT} WHERE UNDEFINED <> ДоговорКонтрагента;"),
    );
    assert_contains(&unequal.sql, "<> decode('0100000000', 'hex'))");

    // A value that cannot hold `Неопределено` never equals it, and that is
    // not an error on the platform either.
    let fixed = tabular_section_snapshot();
    let never = postgres(
        &fixed,
        &format!("ВЫБРАТЬ Ссылка ИЗ {DOCUMENT} ГДЕ ДоговорКонтрагента = НЕОПРЕДЕЛЕНО;"),
    );
    assert_contains(&never.sql, "WHERE FALSE");
    let never = mssql(
        &fixed,
        &format!("ВЫБРАТЬ Ссылка ИЗ {DOCUMENT} ГДЕ ДоговорКонтрагента <> НЕОПРЕДЕЛЕНО;"),
    );
    assert_contains(&never.sql, "WHERE (1 = 1)");

    // The literal fills a branch like `NULL` and leaves the kind to the
    // other operands.
    let branch = postgres(
        &snapshot,
        &format!(
            "ВЫБРАТЬ ВЫБОР КОГДА Ссылка ЕСТЬ NULL ТОГДА НЕОПРЕДЕЛЕНО ИНАЧЕ Ссылка КОНЕЦ КАК Р,
             ЕСТЬNULL(НЕОПРЕДЕЛЕНО, НЕОПРЕДЕЛЕНО) КАК Е ИЗ {DOCUMENT};"
        ),
    );
    assert!(
        matches!(branch.columns[0].kind, ColumnKind::Reference { .. }),
        "{:?}",
        branch.columns[0].kind
    );
    assert_eq!(branch.columns[1].kind, ColumnKind::Undefined);
}

#[test]
fn classifies_bound_parameters() {
    let snapshot = universal_dereferenced_presentation_snapshot();
    let target = snapshot
        .object_id(MetadataKind::Catalog, "ЦентрыФинансовойОтветственности")
        .unwrap();
    let parameters = [
        QueryParameter::new(
            "Ссылка",
            ParameterValue::Reference {
                object: target,
                id: [0x11; 16],
            },
        ),
        QueryParameter::new("Текст", ParameterValue::String("а".to_owned())),
    ];
    let compiled = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile_with(
            &format!(
                "ВЫБРАТЬ ТИПЗНАЧЕНИЯ(&Ссылка) КАК С, ТИПЗНАЧЕНИЯ(&Текст) КАК Т ИЗ {DOCUMENT};"
            ),
            &CompileOptions::new().parameters(&parameters),
        )
        .unwrap();
    assert_contains(&compiled.sql, "decode('080000003e', 'hex') AS \"С\"");
    assert_contains(&compiled.sql, "decode('0500000000', 'hex') AS \"Т\"");
}

#[test]
fn reports_unsupported_type_arguments() {
    let snapshot = universal_dereferenced_presentation_snapshot();
    for (query, kind, message) in [
        (
            format!("ВЫБРАТЬ ТИП(УникальныйИдентификатор) КАК Т ИЗ {DOCUMENT};"),
            QueryDiagnosticKind::Syntax,
            "TYPE expects",
        ),
        (
            format!("ВЫБРАТЬ ТИП(НЕОПРЕДЕЛЕНО) КАК Т ИЗ {DOCUMENT};"),
            QueryDiagnosticKind::Syntax,
            "TYPE expects a type name",
        ),
        (
            format!("ВЫБРАТЬ ТИП(Справочник.Нет) КАК Т ИЗ {DOCUMENT};"),
            QueryDiagnosticKind::UnknownObject,
            "could not be resolved",
        ),
        (
            "ВЫБРАТЬ ТИПЗНАЧЕНИЯ(ProbeAttribute) КАК Т ИЗ Справочник.OpenSdblMetadataProbe;"
                .to_owned(),
            QueryDiagnosticKind::UnsupportedFeature,
            "does not classify",
        ),
        (
            format!("ВЫБРАТЬ ТИПЗНАЧЕНИЯ(Ссылка, 1) КАК Т ИЗ {DOCUMENT};"),
            QueryDiagnosticKind::Syntax,
            "exactly one argument",
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
fn decodes_type_values_by_name() {
    let snapshot = universal_dereferenced_presentation_snapshot();
    let catalog = TypeValue::decode(&[0x08, 0x00, 0x00, 0x00, 0x3e]).unwrap();
    assert_eq!(
        catalog.query_name(&snapshot),
        "Справочник.ЦентрыФинансовойОтветственности"
    );
    assert_eq!(TypeValue::String.query_name(&snapshot), "Строка");
    assert_eq!(TypeValue::Undefined.query_name(&snapshot), "Неопределено");
    assert_eq!(TypeValue::Null.query_name(&snapshot), "Null");
    assert_eq!(
        TypeValue::Reference(9999).query_name(&snapshot),
        "Ссылка.9999"
    );
}

#[test]
fn tests_compound_fields_for_null() {
    let snapshot = universal_dereferenced_presentation_snapshot();
    let compiled = postgres(
        &snapshot,
        &format!("ВЫБРАТЬ Ссылка ИЗ {DOCUMENT} ГДЕ ДоговорКонтрагента ЕСТЬ NULL;"),
    );
    // The discriminator is written for every row of the table itself, so
    // the test only answers true for a missing join row, as on the platform.
    assert_contains(&compiled.sql, "WHERE (\"__src\".\"_fld59_type\" IS NULL)");

    let negated = mssql(
        &snapshot,
        &format!("ВЫБРАТЬ Ссылка ИЗ {DOCUMENT} ГДЕ ДоговорКонтрагента ЕСТЬ НЕ NULL;"),
    );
    assert_contains(&negated.sql, "WHERE ([__src].[_fld59_type] IS NOT NULL)");

    // A single-member field keeps testing its own column.
    let single = postgres(
        &snapshot,
        &format!("ВЫБРАТЬ Ссылка ИЗ {DOCUMENT} ГДЕ Ссылка ЕСТЬ NULL;"),
    );
    assert_contains(&single.sql, "WHERE (\"__src\".\"_idrref\" IS NULL)");
}

#[test]
fn compares_a_composite_field_with_a_value() {
    // The platform tests the `_TYPE` discriminator against the tag of the
    // value and the member that carries it; both were read from the SQL it
    // generates for the same query.
    let snapshot = mixed_composite_snapshot();
    let string = postgres(
        &snapshot,
        "ВЫБРАТЬ Ссылка ИЗ Справочник.OpenSdblMetadataProbe КАК Т ГДЕ Т.ProbeAttribute = \"текст\";",
    );
    assert_contains(
        &string.sql,
        "WHERE ((\"Т\".\"_fld54_type\" = decode('05', 'hex')) AND (\"Т\".\"_fld54_s\" = 'текст'))",
    );

    // A field admitting one reference type stores no `_RTRef` member, so
    // the comparison synthesizes it from the discriminator.
    let reference = postgres(
        &snapshot,
        "ВЫБРАТЬ Ссылка ИЗ Справочник.OpenSdblMetadataProbe КАК Т
         ГДЕ Т.ProbeAttribute = ЗНАЧЕНИЕ(Справочник.OpenSdblMetadataProbe.ПустаяСсылка);",
    );
    assert_contains(
        &reference.sql,
        "(\"Т\".\"_fld54_type\" = decode('08', 'hex'))",
    );
    assert_contains(
        &reference.sql,
        "CASE WHEN \"Т\".\"_fld54_type\" = decode('08', 'hex') THEN decode('00000039', 'hex') \
         WHEN \"Т\".\"_fld54_type\" <> decode('08', 'hex') THEN decode('00000000', 'hex') END",
    );

    // A parameter bound to NULL compares with NULL, as a single-member
    // field already does.
    let parameters = [QueryParameter::new("П", ParameterValue::Null)];
    let unbound = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile_with(
            "ВЫБРАТЬ Ссылка ИЗ Справочник.OpenSdblMetadataProbe КАК Т ГДЕ Т.ProbeAttribute = &П;",
            &CompileOptions::new().parameters(&parameters),
        )
        .unwrap();
    assert_contains(&unbound.sql, "WHERE (\"Т\".\"_fld54_type\" = NULL)");

    // SQL Server spells the same members as binary literals.
    let mssql = mssql(
        &snapshot,
        "ВЫБРАТЬ Ссылка ИЗ Справочник.OpenSdblMetadataProbe КАК Т ГДЕ Т.ProbeAttribute = \"текст\";",
    );
    assert_contains(
        &mssql.sql,
        "WHERE (([Т].[_fld54_type] = 0x05) AND ([Т].[_fld54_s] = N'текст'))",
    );
}

#[test]
fn compares_a_composite_field_with_a_list() {
    // A list is the disjunction of the member comparisons, the way the
    // platform groups its own list by type.
    let snapshot = mixed_composite_snapshot();
    let compiled = postgres(
        &snapshot,
        "ВЫБРАТЬ Ссылка ИЗ Справочник.OpenSdblMetadataProbe КАК Т
         ГДЕ Т.ProbeAttribute В (НЕОПРЕДЕЛЕНО, \"текст\");",
    );
    assert_contains(
        &compiled.sql,
        "WHERE ((\"Т\".\"_fld54_type\" = decode('01', 'hex')) \
         OR ((\"Т\".\"_fld54_type\" = decode('05', 'hex')) AND (\"Т\".\"_fld54_s\" = 'текст')))",
    );

    let negated = postgres(
        &snapshot,
        "ВЫБРАТЬ Ссылка ИЗ Справочник.OpenSdblMetadataProbe КАК Т
         ГДЕ Т.ProbeAttribute НЕ В (\"текст\");",
    );
    assert_contains(&negated.sql, "WHERE (NOT ((\"Т\".\"_fld54_type\"");
}

#[test]
fn compiles_the_simple_case_form() {
    // `ВЫБОР <выражение> КОГДА <значение> ТОГДА …` compares every
    // alternative with the subject. Measured on the platform, including a
    // NULL subject, which matches no alternative.
    let snapshot = universal_dereferenced_presentation_snapshot();
    let compiled = postgres(
        &snapshot,
        &format!(
            "ВЫБРАТЬ ВЫБОР ТИПЗНАЧЕНИЯ(ДоговорКонтрагента)
                КОГДА ТИП(Справочник.ЦентрыФинансовойОтветственности) ТОГДА 1
                ИНАЧЕ 0 КОНЕЦ КАК Р ИЗ {DOCUMENT};"
        ),
    );
    assert_contains(&compiled.sql, "CASE WHEN (");
    assert_contains(&compiled.sql, "decode('080000003e', 'hex')");

    // A reference subject compares by member, as it does in a predicate.
    let reference = postgres(
        &snapshot,
        &format!(
            "ВЫБРАТЬ ВЫБОР Ссылка КОГДА ЗНАЧЕНИЕ(Документ.бит_ДополнительныеУсловияПоДоговору.ПустаяСсылка)
                ТОГДА 1 ИНАЧЕ 0 КОНЕЦ КАК Р ИЗ {DOCUMENT};"
        ),
    );
    assert_contains(&reference.sql, "\"__src\".\"_idrref\" = ");

    // The searched form keeps working.
    let searched = postgres(
        &snapshot,
        &format!("ВЫБРАТЬ ВЫБОР КОГДА Ссылка ЕСТЬ NULL ТОГДА 1 ИНАЧЕ 0 КОНЕЦ КАК Р ИЗ {DOCUMENT};"),
    );
    assert_contains(&searched.sql, "CASE WHEN (\"__src\".\"_idrref\" IS NULL)");
}

#[test]
fn projects_alternatives_of_different_types_by_member() {
    // The platform answers a value of several types and stores it by
    // member; measured on the probe base, both for a string against a
    // reference and for two primitive types.
    let snapshot = universal_dereferenced_presentation_snapshot();
    let compiled = postgres(
        &snapshot,
        &format!(
            "ВЫБРАТЬ ВЫБОР КОГДА Ссылка ЕСТЬ NULL ТОГДА \"нет\" ИНАЧЕ ДоговорКонтрагента КОНЕЦ
                КАК Смесь ИЗ {DOCUMENT};"
        ),
    );
    let labels = compiled
        .columns
        .iter()
        .map(|column| column.label.as_str())
        .collect::<Vec<_>>();
    assert_eq!(labels, ["Смесь", "Смесь_S", "Смесь_TYPE"]);
    assert_contains(&compiled.sql, "THEN 'нет' ELSE ''");
    assert_contains(
        &compiled.sql,
        "THEN decode('05', 'hex') ELSE decode('08', 'hex') END AS \"Смесь_TYPE\"",
    );
    assert!(
        matches!(
            compiled.columns[0].kind,
            ColumnKind::Reference {
                runtime_typed: true,
                ..
            }
        ),
        "{:?}",
        compiled.columns[0].kind
    );

    // Alternatives that agree on a kind keep their single column.
    let single = postgres(
        &snapshot,
        &format!("ВЫБРАТЬ ВЫБОР КОГДА Ссылка ЕСТЬ NULL ТОГДА 1 ИНАЧЕ 2 КОНЕЦ КАК Ч ИЗ {DOCUMENT};"),
    );
    assert_eq!(single.columns.len(), 1);
}
