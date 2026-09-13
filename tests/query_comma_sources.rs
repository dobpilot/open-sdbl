//! Comma-separated source lists: `ИЗ А, Б …` rendered as `CROSS JOIN`.

mod support;

use support::*;

use open_sdbl::metadata::MetadataSnapshot;
use open_sdbl::query::{
    Backend, CompileOptions, CompiledQuery, MsSqlBackend, ParameterValue, PostgresBackend,
    QueryCompiler, QueryDiagnostic, QueryDiagnosticKind, QueryParameter, SessionParameters,
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

#[test]
fn renders_comma_sources_as_a_cross_join_chain() {
    let snapshot = reference_snapshot();
    let query = "ВЫБРАТЬ p.Code КАК К, o.Code КАК О
         ИЗ Справочник.OpenSdblMetadataProbe КАК p, Справочник.Организации КАК o
         ГДЕ p.Code = o.Code
         УПОРЯДОЧИТЬ ПО К;";
    let compiled = postgres(&snapshot, query);
    assert_contains(
        &compiled.sql,
        "FROM \"_reference53\" AS \"p\" CROSS JOIN \"_reference57\" AS \"o\" WHERE (\"p\".\"_code\" = \"o\".\"_code\")",
    );
    assert_eq!(compiled.columns.len(), 2);
    let mssql = compile(&snapshot, MsSqlBackend::new(0).unwrap(), query).unwrap();
    assert_contains(
        &mssql.sql,
        "FROM [_reference53] AS [p] CROSS JOIN [_reference57] AS [o] WHERE ([p].[_code] = [o].[_code]) ORDER BY 1 ASC",
    );

    let three = postgres(
        &snapshot,
        "SELECT a.Code, b.Code, c.Code
         FROM Catalog.OpenSdblMetadataProbe AS a, Catalog.Организации AS b, Catalog.Организации AS c
         WHERE a.Code = b.Code AND b.Code = c.Code;",
    );
    assert_contains(
        &three.sql,
        "AS \"a\" CROSS JOIN \"_reference57\" AS \"b\" CROSS JOIN \"_reference57\" AS \"c\"",
    );
}

#[test]
fn joins_stay_attached_to_their_comma_element() {
    let snapshot = reference_snapshot();
    let after = postgres(
        &snapshot,
        "SELECT a.Code, b.Code, c.Code
         FROM Catalog.OpenSdblMetadataProbe AS a, Catalog.Организации AS b
         LEFT JOIN Catalog.Организации AS c ON c.Code = b.Code
         WHERE a.Code = b.Code;",
    );
    assert_contains(
        &after.sql,
        "AS \"a\" CROSS JOIN \"_reference57\" AS \"b\" LEFT JOIN \"_reference57\" AS \"c\" ON \"c\".\"_code\" = \"b\".\"_code\" WHERE",
    );

    let before = postgres(
        &snapshot,
        "SELECT a.Code, c.Code, b.Code
         FROM Catalog.OpenSdblMetadataProbe AS a
         LEFT JOIN Catalog.Организации AS c ON c.Code = a.Code, Catalog.Организации AS b
         WHERE a.Code = b.Code;",
    );
    assert_contains(
        &before.sql,
        "AS \"a\" LEFT JOIN \"_reference57\" AS \"c\" ON \"c\".\"_code\" = \"a\".\"_code\" CROSS JOIN \"_reference57\" AS \"b\"",
    );

    let invisible = compile(
        &snapshot,
        PostgresBackend,
        "SELECT a.Code FROM Catalog.OpenSdblMetadataProbe AS a, Catalog.Организации AS b
         LEFT JOIN Catalog.Организации AS c ON c.Code = a.Code;",
    )
    .unwrap_err();
    assert_eq!(invisible.kind(), QueryDiagnosticKind::UnknownField);
    assert!(invisible.message().contains("not visible from this join"));
}

#[test]
fn accepts_derived_virtual_temporary_and_constant_sources() {
    let snapshot = accumulation_register_snapshot();
    let mixed = postgres(
        &snapshot,
        "ВЫБРАТЬ Т.Номенклатура, О.КоличествоОстаток, Н.Ч
         ИЗ РегистрНакопления.Остатки КАК Т,
            РегистрНакопления.Остатки.Остатки() КАК О,
            (ВЫБРАТЬ 1 КАК Ч) КАК Н
         ГДЕ Т.Номенклатура = О.Номенклатура;",
    );
    assert_contains(&mixed.sql, "CROSS JOIN (SELECT");
    assert_contains(
        &mixed.sql,
        "AS \"О\" CROSS JOIN (SELECT 1 AS \"Ч\") AS \"Н\"",
    );

    let batch = postgres(
        &snapshot,
        "ВЫБРАТЬ Номенклатура КАК Н ПОМЕСТИТЬ ВТ ИЗ РегистрНакопления.Остатки;
         ВЫБРАТЬ Т.Номенклатура, Х.Н ИЗ РегистрНакопления.Остатки КАК Т, ВТ КАК Х ГДЕ Т.Номенклатура = Х.Н;",
    );
    assert_contains(&batch.sql, "CROSS JOIN \"vt1\" AS \"Х\"");
}

#[test]
fn filters_comma_sources_by_separators_like_the_base() {
    let snapshot = separators_snapshot();
    let mut session = SessionParameters::new();
    session.set(QueryParameter::new(
        "ЗначениеРазделителя",
        ParameterValue::Number {
            unscaled: 7,
            scale: 0,
        },
    ));
    let compiled = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile_with(
            "ВЫБРАТЬ А.Артикул, Б.Артикул ИЗ Справочник.Товары КАК А, Справочник.Товары КАК Б ГДЕ А.Поставщик = Б.Ссылка;",
            &CompileOptions::new().session(&session),
        )
        .unwrap();
    assert_contains(
        &compiled.sql,
        "WHERE \"А\".\"_fld56\" = 7 AND \"А\".\"_fld57\" = 7 AND \"Б\".\"_fld56\" = 7 AND \"Б\".\"_fld57\" = 7 AND (\"А\".\"_fld55rref\" = \"Б\".\"_idrref\")",
    );

    let right = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile_with(
            "ВЫБРАТЬ А.Артикул, Б.Артикул, Г.Артикул ИЗ Справочник.Товары КАК А, Справочник.Товары КАК Б
             ПРАВОЕ СОЕДИНЕНИЕ Справочник.Товары КАК Г ПО Г.Поставщик = Б.Ссылка;",
            &CompileOptions::new().session(&session),
        )
        .unwrap();
    assert_contains(
        &right.sql,
        "RIGHT JOIN \"_reference53\" AS \"Г\" ON \"Г\".\"_fld55rref\" = \"Б\".\"_idrref\" AND \"А\".\"_fld56\" = 7 AND \"А\".\"_fld57\" = 7 AND \"Б\".\"_fld56\" = 7 AND \"Б\".\"_fld57\" = 7 WHERE \"Г\".\"_fld56\" = 7 AND \"Г\".\"_fld57\" = 7",
    );
}

#[test]
fn keeps_the_full_join_wildcard_and_ambiguity_refusals() {
    let snapshot = reference_snapshot();
    for (query, message) in [
        (
            "SELECT a.Code, b.Code FROM Catalog.OpenSdblMetadataProbe AS a, Catalog.Организации AS b
             FULL JOIN Catalog.Организации AS c ON c.Code = b.Code;",
            "FULL JOIN must be the only join",
        ),
        (
            "SELECT * FROM Catalog.OpenSdblMetadataProbe AS a, Catalog.Организации AS b;",
            "wildcard projection over several sources",
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

    let ambiguous = compile(
        &snapshot,
        PostgresBackend,
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe AS a, Catalog.Организации AS b;",
    )
    .unwrap_err();
    assert!(ambiguous.message().contains("ambiguous"), "{ambiguous}");
}
