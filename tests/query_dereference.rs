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

#[test]
fn dereferences_standard_fields_through_a_composite_reference() {
    // SchemaStorage names no target for a reference of several tables, so
    // the candidates are scanned; standard fields are not attributes and
    // have to be recognized by name.
    let snapshot = universal_dereferenced_presentation_snapshot();
    let compiled = postgres(
        &snapshot,
        "ВЫБРАТЬ ДоговорКонтрагента.Ссылка КАК С
         ИЗ Документ.бит_ДополнительныеУсловияПоДоговору;",
    );
    // Each candidate target is joined under its own type guard.
    assert_contains(&compiled.sql, "\"__src\".\"_fld59_rtref\"");
    assert!(compiled.sql.contains("LEFT JOIN"), "{}", compiled.sql);

    let unknown = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile(
            "ВЫБРАТЬ ДоговорКонтрагента.НетТакого КАК Н
             ИЗ Документ.бит_ДополнительныеУсловияПоДоговору;",
        )
        .unwrap_err();
    assert!(unknown.message().contains("was not found"), "{unknown}");
}

#[test]
fn resolves_the_computed_standard_fields() {
    fn compile_separated(snapshot: &MetadataSnapshot, source: &str) -> CompiledQuery {
        let mut session = open_sdbl::query::SessionParameters::new();
        session.set(open_sdbl::query::QueryParameter::new(
            "ЗначениеРазделителя",
            open_sdbl::query::ParameterValue::Number {
                unscaled: 0,
                scale: 0,
            },
        ));
        session.set(open_sdbl::query::QueryParameter::new(
            "ИспользованиеРазделителя",
            open_sdbl::query::ParameterValue::Boolean(false),
        ));
        QueryCompiler::new(snapshot, PostgresBackend)
            .compile_with(
                source,
                &open_sdbl::query::CompileOptions::new().session(&session),
            )
            .unwrap_or_else(|error| panic!("{source}: {error}"))
    }

    // `ЭтоГруппа` is the negation of the stored `Folder` column, which is
    // true for an item, and `Предопределенный` says the item has a
    // predefined identity.
    let snapshot = support::demo_resolved_at(
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/demo"),
    )
    .snapshot;
    let compiled = compile_separated(
        &snapshot,
        "ВЫБРАТЬ Т.Наименование КАК Имя, Т.ЭтоГруппа КАК Гр, Т.Предопределенный КАК Пред
         ИЗ Справочник.ГруппыДоступа КАК Т ГДЕ Т.ЭтоГруппа;",
    );
    assert_contains(&compiled.sql, "(\"Т\".\"_folder\" = FALSE) AS \"Гр\"");
    assert_contains(
        &compiled.sql,
        "(\"Т\".\"_predefinedid\" <> decode('00000000000000000000000000000000', 'hex')) AS \"Пред\"",
    );
    assert_contains(&compiled.sql, "AND (\"Т\".\"_folder\" = FALSE)");
    assert_eq!(
        compiled.columns[1].kind,
        open_sdbl::query::ColumnKind::Boolean
    );

    // SQL Server has no boolean type, so the value is spelled as a bit.
    let mut session = open_sdbl::query::SessionParameters::new();
    session.set(open_sdbl::query::QueryParameter::new(
        "ЗначениеРазделителя",
        open_sdbl::query::ParameterValue::Number {
            unscaled: 0,
            scale: 0,
        },
    ));
    session.set(open_sdbl::query::QueryParameter::new(
        "ИспользованиеРазделителя",
        open_sdbl::query::ParameterValue::Boolean(false),
    ));
    let mssql = QueryCompiler::new(&snapshot, MsSqlBackend::new(0).unwrap())
        .compile_with(
            "ВЫБРАТЬ Т.ЭтоГруппа КАК Гр ИЗ Справочник.ГруппыДоступа КАК Т;",
            &open_sdbl::query::CompileOptions::new().session(&session),
        )
        .unwrap();
    assert_contains(
        &mssql.sql,
        "CASE WHEN [Т].[_folder] = 0x00 THEN 0x01 ELSE 0x00 END AS [Гр]",
    );

    // They also answer through a reference.
    let dereferenced = compile_separated(
        &snapshot,
        "ВЫБРАТЬ Т.Родитель.ЭтоГруппа КАК Гр ИЗ Справочник.ГруппыДоступа КАК Т;",
    );
    assert_contains(
        &dereferenced.sql,
        "(\"__ref1\".\"_folder\" = FALSE) AS \"Гр\"",
    );
}
