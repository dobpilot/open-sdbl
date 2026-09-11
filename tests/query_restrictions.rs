//! `РАЗРЕШЕННЫЕ` statements, access restrictions, and session parameters.

mod support;

use support::*;

use open_sdbl::metadata::{MetadataSnapshot, ObjectId};
use open_sdbl::query::{
    AccessRestriction, Backend, CompileOptions, CompiledQuery, MsSqlBackend, ParameterValue,
    PostgresBackend, QueryCompiler, QueryDiagnostic, QueryDiagnosticKind, QueryParameter,
    RestrictionTarget, SessionParameters, TempTablesManager, find_metadata_object,
};

fn object_id(snapshot: &MetadataSnapshot, name: &str) -> ObjectId {
    ObjectId::from(&find_metadata_object(snapshot, name).unwrap().guid)
}

fn mssql() -> MsSqlBackend {
    MsSqlBackend::new(0).unwrap()
}

fn compile<B: Backend>(
    snapshot: &MetadataSnapshot,
    backend: B,
    source: &str,
    options: &CompileOptions<'_>,
) -> Result<CompiledQuery, QueryDiagnostic> {
    QueryCompiler::new(snapshot, backend).compile_with(source, options)
}

/// Compiles on both dialects, checks that they agree on success and
/// diagnostic kind, and returns the PostgreSQL outcome.
fn compile_both(
    snapshot: &MetadataSnapshot,
    source: &str,
    options: &CompileOptions<'_>,
) -> Result<CompiledQuery, QueryDiagnostic> {
    let postgres = compile(snapshot, PostgresBackend, source, options);
    let mssql = compile(snapshot, mssql(), source, options);
    match (&postgres, &mssql) {
        (Ok(_), Ok(_)) => {}
        (Err(left), Err(right)) => {
            assert_eq!(left.kind(), right.kind(), "{source}");
            assert_eq!(
                (left.line(), left.column()),
                (right.line(), right.column()),
                "{source}"
            );
        }
        _ => panic!("backend outcomes differ for {source}: {postgres:?} / {mssql:?}"),
    }
    postgres
}

fn restriction(snapshot: &MetadataSnapshot, object: &str, condition: &str) -> AccessRestriction {
    AccessRestriction::new(object_id(snapshot, object), condition)
}

fn session(parameters: &[(&str, ParameterValue)]) -> SessionParameters {
    let mut session = SessionParameters::new();
    for (name, value) in parameters {
        session.set(QueryParameter::new(*name, value.clone()));
    }
    session
}

#[test]
fn accepts_the_keyword_before_distinct_and_top_without_changing_sql() {
    let snapshot = snapshot();
    let restricted = compile_both(
        &snapshot,
        "ВЫБРАТЬ РАЗРЕШЕННЫЕ РАЗЛИЧНЫЕ ПЕРВЫЕ 10 Code ИЗ Catalog.OpenSdblMetadataProbe;",
        &CompileOptions::new(),
    )
    .unwrap();
    let plain = compile_both(
        &snapshot,
        "ВЫБРАТЬ РАЗЛИЧНЫЕ ПЕРВЫЕ 10 Code ИЗ Catalog.OpenSdblMetadataProbe;",
        &CompileOptions::new(),
    )
    .unwrap();

    assert_eq!(restricted.sql, plain.sql);
    assert!(restricted.sql.starts_with("SELECT DISTINCT"));
    assert!(restricted.sql.ends_with("LIMIT 10"));

    let english = compile_both(
        &snapshot,
        "SELECT ALLOWED Code FROM Catalog.OpenSdblMetadataProbe;",
        &CompileOptions::new(),
    )
    .unwrap();
    assert!(!english.sql.contains("__restricted"));
}

#[test]
fn rejects_the_keyword_outside_the_first_top_level_branch() {
    let snapshot = snapshot();
    let union = compile_both(
        &snapshot,
        "ВЫБРАТЬ Code ИЗ Catalog.OpenSdblMetadataProbe ОБЪЕДИНИТЬ ВСЕ ВЫБРАТЬ РАЗРЕШЕННЫЕ Code ИЗ Catalog.OpenSdblMetadataProbe;",
        &CompileOptions::new(),
    )
    .unwrap_err();
    assert_eq!(union.kind(), QueryDiagnosticKind::Syntax);
    assert_eq!((union.line(), union.column()), (1, 70));

    let nested = compile_both(
        &snapshot,
        "ВЫБРАТЬ Вл.Code ИЗ (ВЫБРАТЬ РАЗРЕШЕННЫЕ Code ИЗ Catalog.OpenSdblMetadataProbe) КАК Вл;",
        &CompileOptions::new(),
    )
    .unwrap_err();
    assert_eq!(nested.kind(), QueryDiagnosticKind::Syntax);
    assert_eq!((nested.line(), nested.column()), (1, 29));

    let subquery = compile_both(
        &snapshot,
        "ВЫБРАТЬ Code ИЗ Catalog.OpenSdblMetadataProbe ГДЕ Code В (ВЫБРАТЬ РАЗРЕШЕННЫЕ Code ИЗ Catalog.OpenSdblMetadataProbe);",
        &CompileOptions::new(),
    )
    .unwrap_err();
    assert_eq!(subquery.kind(), QueryDiagnosticKind::Syntax);
}

#[test]
fn requests_every_target_of_allowed_statements() {
    let snapshot = tabular_section_snapshot();
    let document = object_id(&snapshot, "Документ.бит_ДополнительныеУсловияПоДоговору");
    let catalog = object_id(&snapshot, "Справочник.ЦентрыФинансовойОтветственности");
    let source = "ВЫБРАТЬ РАЗРЕШЕННЫЕ Т.Сумма
         ИЗ Документ.бит_ДополнительныеУсловияПоДоговору.ГрафикНачислений КАК Т
         ЛЕВОЕ СОЕДИНЕНИЕ Справочник.ЦентрыФинансовойОтветственности КАК Ц ПО Т.ЦФО = Ц.Ссылка
         ГДЕ Т.Ссылка В (ВЫБРАТЬ Д.Ссылка ИЗ Документ.бит_ДополнительныеУсловияПоДоговору КАК Д)";
    let postgres = QueryCompiler::new(&snapshot, PostgresBackend)
        .prepare(source)
        .unwrap();
    let mssql = QueryCompiler::new(&snapshot, mssql())
        .prepare(source)
        .unwrap();
    assert_eq!(postgres.restriction_request(), mssql.restriction_request());

    let mut expected = vec![
        RestrictionTarget {
            object: document,
            table_part: Some("ГрафикНачислений".to_owned()),
        },
        RestrictionTarget {
            object: catalog,
            table_part: None,
        },
        RestrictionTarget {
            object: document,
            table_part: None,
        },
    ];
    expected.sort();
    assert_eq!(postgres.restriction_request().targets, expected);
    assert!(postgres.presentation_request().targets.is_empty());

    let unrestricted = QueryCompiler::new(&snapshot, PostgresBackend)
        .prepare(
            "ВЫБРАТЬ Т.Сумма ИЗ Документ.бит_ДополнительныеУсловияПоДоговору.ГрафикНачислений КАК Т;
             ВЫБРАТЬ Ц.Ссылка ИЗ Справочник.ЦентрыФинансовойОтветственности КАК Ц",
        )
        .unwrap();
    assert!(unrestricted.restriction_request().targets.is_empty());

    let mixed = QueryCompiler::new(&snapshot, PostgresBackend)
        .prepare(
            "ВЫБРАТЬ Т.Сумма ИЗ Документ.бит_ДополнительныеУсловияПоДоговору.ГрафикНачислений КАК Т;
             ВЫБРАТЬ РАЗРЕШЕННЫЕ Ц.Ссылка ИЗ Справочник.ЦентрыФинансовойОтветственности КАК Ц",
        )
        .unwrap();
    assert_eq!(
        mixed.restriction_request().targets,
        [RestrictionTarget {
            object: catalog,
            table_part: None,
        }]
    );
}

#[test]
fn wraps_a_restricted_source_in_a_derived_table_with_dereference_and_session_value() {
    let snapshot = reference_snapshot();
    let restrictions = [restriction(
        &snapshot,
        "Catalog.OpenSdblMetadataProbe",
        "Организация.Code = &Орг ИЛИ Code = \"free\"",
    )];
    let session = session(&[("Орг", ParameterValue::String("HQ".to_owned()))]);
    let options = CompileOptions::new()
        .restrictions(&restrictions)
        .session(&session);
    let source = "ВЫБРАТЬ РАЗРЕШЕННЫЕ p.Code ИЗ Catalog.OpenSdblMetadataProbe КАК p ГДЕ p.Code <> \"X\" УПОРЯДОЧИТЬ ПО p.Code";

    let compiled = compile_both(&snapshot, source, &options).unwrap();
    assert!(
        compiled.sql.contains(
            "FROM (SELECT \"__restricted\".\"_idrref\" AS \"_idrref\", \"__restricted\".\"_code\" AS \"_code\", \"__restricted\".\"_date_time\" AS \"_date_time\", \"__restricted\".\"_fld54\" AS \"_fld54\" FROM \"_reference53\" AS \"__restricted\" LEFT JOIN \"_reference57\" AS \"__ref1\" ON \"__restricted\".\"_fld54\" = \"__ref1\".\"_idrref\" WHERE ((\"__ref1\".\"_code\" = 'HQ') OR (\"__restricted\".\"_code\" = 'free'))) AS \"p\""
        ),
        "{}",
        compiled.sql
    );
    assert!(compiled.sql.contains("WHERE (\"p\".\"_code\" <> 'X')"));
    assert!(compiled.sql.ends_with("ORDER BY \"p\".\"_code\" ASC"));

    let mssql = compile(&snapshot, mssql(), source, &options).unwrap();
    assert!(mssql.sql.contains("FROM [_reference53] AS [__restricted]"));
    assert!(mssql.sql.contains("WHERE (([__ref1].[_code] = N'HQ')"));
    assert!(mssql.sql.contains(") AS [p] WHERE ([p].[_code] <> N'X')"));

    let plain = compile_both(&snapshot, source, &CompileOptions::new().session(&session)).unwrap();
    let without_keyword = compile_both(
        &snapshot,
        &source.replace("РАЗРЕШЕННЫЕ ", ""),
        &CompileOptions::new().session(&session),
    )
    .unwrap();
    assert_eq!(plain.sql, without_keyword.sql);
    assert!(!plain.sql.contains("__restricted"));
}

#[test]
fn restricts_joined_sources_independently() {
    let snapshot = reference_snapshot();
    let restrictions = [restriction(
        &snapshot,
        "Catalog.Организации",
        "Code В (\"A\", \"B\")",
    )];
    let options = CompileOptions::new().restrictions(&restrictions);
    let compiled = compile_both(
        &snapshot,
        "ВЫБРАТЬ РАЗРЕШЕННЫЕ p.Code, t.Code КАК Организация
         ИЗ Catalog.OpenSdblMetadataProbe КАК p
         ЛЕВОЕ СОЕДИНЕНИЕ Catalog.Организации КАК t ПО p.Code = t.Code",
        &options,
    )
    .unwrap();

    assert!(
        compiled
            .sql
            .contains("FROM \"_reference53\" AS \"p\" LEFT JOIN (SELECT")
    );
    assert!(compiled.sql.contains(
        "FROM \"_reference57\" AS \"__restricted\" WHERE (\"__restricted\".\"_code\" IN ('A', 'B'))) AS \"t\" ON \"p\".\"_code\" = \"t\".\"_code\""
    ));

    let inner = compile_both(
        &snapshot,
        "ВЫБРАТЬ РАЗРЕШЕННЫЕ p.Code
         ИЗ Catalog.OpenSdblMetadataProbe КАК p
         ПОЛНОЕ СОЕДИНЕНИЕ Catalog.Организации КАК t ПО p.Code = t.Code",
        &options,
    )
    .unwrap();
    assert_eq!(inner.sql.matches("AS \"__restricted\"").count(), 2);
}

#[test]
fn restricts_tabular_sections_by_section_name() {
    let snapshot = tabular_section_snapshot();
    let document = object_id(&snapshot, "Документ.бит_ДополнительныеУсловияПоДоговору");
    let restrictions =
        [AccessRestriction::new(document, "Сумма > 0").table_part("графикначислений")];
    let options = CompileOptions::new().restrictions(&restrictions);
    let compiled = compile_both(
        &snapshot,
        "ВЫБРАТЬ РАЗРЕШЕННЫЕ Т.Сумма ИЗ Документ.бит_ДополнительныеУсловияПоДоговору.ГрафикНачислений КАК Т",
        &options,
    )
    .unwrap();
    assert!(
        compiled.sql.contains(
            "FROM \"_document53_vt54X1\" AS \"__restricted\" WHERE (\"__restricted\".\"_fld57\" > 0)) AS \"Т\""
        ),
        "{}",
        compiled.sql
    );

    let owner_only = [AccessRestriction::new(document, "Сумма > 0")];
    let unused = compile_both(
        &snapshot,
        "ВЫБРАТЬ РАЗРЕШЕННЫЕ Т.Сумма ИЗ Документ.бит_ДополнительныеУсловияПоДоговору.ГрафикНачислений КАК Т",
        &CompileOptions::new().restrictions(&owner_only),
    )
    .unwrap_err();
    assert_eq!(unused.kind(), QueryDiagnosticKind::Restriction);
    assert!(
        unused.message().contains(
            "restriction of Документ.бит_ДополнительныеУсловияПоДоговору is supplied but no ALLOWED statement reads it"
        ),
        "{}",
        unused.message()
    );
}

#[test]
fn conjoins_restrictions_into_virtual_table_predicates() {
    let snapshot = accumulation_register_snapshot();
    let restrictions = [restriction(
        &snapshot,
        "РегистрНакопления.Остатки",
        "Номенклатура ЕСТЬ НЕ NULL",
    )];
    let options = CompileOptions::new().restrictions(&restrictions);
    let current = compile_both(
        &snapshot,
        "ВЫБРАТЬ РАЗРЕШЕННЫЕ КоличествоОстаток ИЗ РегистрНакопления.Остатки.Остатки()",
        &options,
    )
    .unwrap();
    assert!(
        current
            .sql
            .contains("AND (\"__totals_base\".\"_fld54\" IS NOT NULL) GROUP BY")
    );
    assert!(!current.sql.contains("__restricted"));

    let historical = compile_both(
        &snapshot,
        "ВЫБРАТЬ РАЗРЕШЕННЫЕ КоличествоОстаток ИЗ РегистрНакопления.Остатки.Остатки(\"2026-09-01\", Номенклатура ЕСТЬ NULL)",
        &options,
    )
    .unwrap();
    assert!(historical.sql.contains(
        "(\"__totals_base\".\"_fld54\" IS NOT NULL) AND (\"__totals_base\".\"_fld54\" IS NULL)"
    ));
    assert!(historical.sql.contains(
        "(\"__movement_base\".\"_fld54\" IS NOT NULL) AND (\"__movement_base\".\"_fld54\" IS NULL)"
    ));

    let turnovers = compile_both(
        &snapshot,
        "ВЫБРАТЬ РАЗРЕШЕННЫЕ КоличествоОборот ИЗ РегистрНакопления.Остатки.Обороты()",
        &options,
    )
    .unwrap();
    assert!(
        turnovers
            .sql
            .contains("AND (\"__aggregate_base\".\"_fld54\" IS NOT NULL)")
    );

    let plain = compile_both(
        &snapshot,
        "ВЫБРАТЬ РАЗРЕШЕННЫЕ Номенклатура ИЗ РегистрНакопления.Остатки",
        &options,
    )
    .unwrap();
    assert!(plain.sql.contains("FROM \"_accumrg53\" AS \"__restricted\" WHERE (\"__restricted\".\"_fld54\" IS NOT NULL)) AS \"__src\""));

    let register = information_register_snapshot();
    let restrictions = [restriction(
        &register,
        "РегистрСведений.Prices",
        "ProbeAttribute ЕСТЬ НЕ NULL",
    )];
    let slice = compile_both(
        &register,
        "ВЫБРАТЬ РАЗРЕШЕННЫЕ Period ИЗ РегистрСведений.Prices.СрезПоследних(, Period > \"2020-01-01\")",
        &CompileOptions::new().restrictions(&restrictions),
    )
    .unwrap();
    assert!(
        slice.sql.contains(
            "WHERE (\"__slice_base\".\"_period\" > '2020-01-01') AND (\"__slice_base\".\"_fld54\" IS NOT NULL)) AS \"__slice_ranked\""
        ),
        "{}",
        slice.sql
    );
}

#[test]
fn stores_filtered_rows_in_temporary_tables_and_leaves_other_statements_alone() {
    let snapshot = snapshot();
    let restrictions = [restriction(
        &snapshot,
        "Catalog.OpenSdblMetadataProbe",
        "Code <> \"secret\"",
    )];
    let options = CompileOptions::new().restrictions(&restrictions);
    let mut manager = TempTablesManager::new();
    let compiled = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile_batch(
            "ВЫБРАТЬ РАЗРЕШЕННЫЕ Code КАК К ПОМЕСТИТЬ ВТ ИЗ Catalog.OpenSdblMetadataProbe;
             ВЫБРАТЬ Code ИЗ Catalog.OpenSdblMetadataProbe;
             ВЫБРАТЬ Т.К ИЗ ВТ КАК Т",
            &options,
            &mut manager,
        )
        .unwrap()
        .unwrap();

    assert!(compiled.sql.starts_with("WITH \"vt1\" AS (SELECT"));
    assert!(compiled.sql.contains("FROM \"_reference53\" AS \"__restricted\" WHERE (\"__restricted\".\"_code\" <> 'secret')) AS \"__src\")"));
    assert!(compiled.sql.ends_with("FROM \"vt1\" AS \"Т\""));
    assert_eq!(compiled.sql.matches("__restricted").count(), 6);

    let unrestricted = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile_batch(
            "ВЫБРАТЬ РАЗРЕШЕННЫЕ Code КАК К ПОМЕСТИТЬ ВТ2 ИЗ Catalog.OpenSdblMetadataProbe;
             ВЫБРАТЬ Code ИЗ Catalog.OpenSdblMetadataProbe",
            &options,
            &mut manager,
        )
        .unwrap()
        .unwrap();
    assert!(
        unrestricted
            .sql
            .ends_with("FROM \"_reference53\" AS \"__src\"")
    );
}

#[test]
fn resolves_session_parameters_after_query_parameters() {
    let snapshot = snapshot();
    let session = session(&[
        ("Код", ParameterValue::String("S".to_owned())),
        ("Лишний", ParameterValue::Null),
    ]);
    let source = "ВЫБРАТЬ Code ИЗ Catalog.OpenSdblMetadataProbe ГДЕ Code = &Код";

    let fallback =
        compile_both(&snapshot, source, &CompileOptions::new().session(&session)).unwrap();
    assert!(fallback.sql.ends_with("WHERE (\"__src\".\"_code\" = 'S')"));

    let query = [QueryParameter::new(
        "код",
        ParameterValue::String("Q".to_owned()),
    )];
    let overridden = compile_both(
        &snapshot,
        source,
        &CompileOptions::new().session(&session).parameters(&query),
    )
    .unwrap();
    assert!(
        overridden
            .sql
            .ends_with("WHERE (\"__src\".\"_code\" = 'Q')")
    );

    let missing = compile_both(&snapshot, source, &CompileOptions::new()).unwrap_err();
    assert_eq!(missing.kind(), QueryDiagnosticKind::Parameter);

    let restrictions = [restriction(
        &snapshot,
        "Catalog.OpenSdblMetadataProbe",
        "Code = &Код",
    )];
    let hidden = compile_both(
        &snapshot,
        "ВЫБРАТЬ РАЗРЕШЕННЫЕ Code ИЗ Catalog.OpenSdblMetadataProbe ГДЕ Code = &Код",
        &CompileOptions::new()
            .parameters(&query)
            .restrictions(&restrictions),
    )
    .unwrap_err();
    assert_eq!(hidden.kind(), QueryDiagnosticKind::Restriction);
    assert!(hidden.message().contains("parameter \"&Код\" has no value"));
    assert_eq!((hidden.line(), hidden.column()), (1, 8));

    let visible = compile_both(
        &snapshot,
        "ВЫБРАТЬ РАЗРЕШЕННЫЕ Code ИЗ Catalog.OpenSdblMetadataProbe ГДЕ Code = &Код",
        &CompileOptions::new()
            .parameters(&query)
            .session(&session)
            .restrictions(&restrictions),
    )
    .unwrap();
    assert!(visible.sql.contains(
        "WHERE (\"__restricted\".\"_code\" = 'S')) AS \"__src\" WHERE (\"__src\".\"_code\" = 'Q')"
    ));
}

#[test]
fn diagnoses_restriction_failures_inside_the_restriction_text() {
    let snapshot = reference_snapshot();
    let source = "ВЫБРАТЬ РАЗРЕШЕННЫЕ Code ИЗ Catalog.OpenSdblMetadataProbe";

    let unknown = [restriction(
        &snapshot,
        "Catalog.OpenSdblMetadataProbe",
        "Code = \"A\" И Нет = 1",
    )];
    let error = compile_both(
        &snapshot,
        source,
        &CompileOptions::new().restrictions(&unknown),
    )
    .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::Restriction);
    assert_eq!((error.line(), error.column()), (1, 14));
    assert!(
        error
            .message()
            .starts_with("restriction of Справочник.OpenSdblMetadataProbe: field \"Нет\""),
        "{}",
        error.message()
    );

    let trailing = [restriction(
        &snapshot,
        "Catalog.OpenSdblMetadataProbe",
        "Code = \"A\" Code",
    )];
    let error = compile_both(
        &snapshot,
        source,
        &CompileOptions::new().restrictions(&trailing),
    )
    .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::Restriction);
    assert_eq!((error.line(), error.column()), (1, 12));

    let empty = [restriction(
        &snapshot,
        "Catalog.OpenSdblMetadataProbe",
        "  ",
    )];
    let error = compile_both(
        &snapshot,
        source,
        &CompileOptions::new().restrictions(&empty),
    )
    .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::Restriction);
    assert!(error.message().contains("condition is empty"));

    let malformed = [restriction(
        &snapshot,
        "Catalog.OpenSdblMetadataProbe",
        "Code = \"A",
    )];
    let error = compile_both(
        &snapshot,
        source,
        &CompileOptions::new().restrictions(&malformed),
    )
    .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::Restriction);

    let duplicate = [
        restriction(&snapshot, "Catalog.OpenSdblMetadataProbe", "Code = \"A\""),
        restriction(&snapshot, "Catalog.OpenSdblMetadataProbe", "Code = \"B\""),
    ];
    let error = compile_both(
        &snapshot,
        source,
        &CompileOptions::new().restrictions(&duplicate),
    )
    .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::Restriction);
    assert!(error.message().contains("more than once"));

    let unused = [restriction(
        &snapshot,
        "Catalog.Организации",
        "Code = \"A\"",
    )];
    let error = compile_both(
        &snapshot,
        source,
        &CompileOptions::new().restrictions(&unused),
    )
    .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::Restriction);
    assert!(error.message().contains("Справочник.Организации"));

    let without_keyword = compile_both(
        &snapshot,
        "ВЫБРАТЬ Code ИЗ Catalog.OpenSdblMetadataProbe",
        &CompileOptions::new().restrictions(&unknown),
    )
    .unwrap_err();
    assert_eq!(without_keyword.kind(), QueryDiagnosticKind::Restriction);
    assert!(
        without_keyword
            .message()
            .contains("no ALLOWED statement reads it")
    );

    let prepared = QueryCompiler::new(&snapshot, PostgresBackend)
        .prepare(source)
        .unwrap();
    assert_eq!(prepared.restriction_request().targets.len(), 1);
    let error = prepared
        .compile_with(&snapshot, &CompileOptions::new().restrictions(&unknown))
        .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::Restriction);
}

#[test]
fn keeps_nested_queries_of_a_restriction_unrestricted() {
    let snapshot = reference_snapshot();
    let restrictions = [
        restriction(
            &snapshot,
            "Catalog.OpenSdblMetadataProbe",
            "Ссылка В (ВЫБРАТЬ О.Ссылка ИЗ Catalog.Организации КАК О ГДЕ О.Code = \"HQ\")",
        ),
        restriction(&snapshot, "Catalog.Организации", "Code <> \"hidden\""),
    ];
    let compiled = compile_both(
        &snapshot,
        "ВЫБРАТЬ РАЗРЕШЕННЫЕ p.Code ИЗ Catalog.OpenSdblMetadataProbe КАК p
         ЛЕВОЕ СОЕДИНЕНИЕ Catalog.Организации КАК o ПО p.Организация = o.Ссылка",
        &CompileOptions::new().restrictions(&restrictions),
    )
    .unwrap();

    assert!(compiled.sql.contains(
        "WHERE (\"__restricted\".\"_idrref\" IN (SELECT \"О\".\"_idrref\" AS \"ID\" FROM \"_reference57\" AS \"О\" WHERE (\"О\".\"_code\" = 'HQ')))) AS \"p\""
    ), "{}", compiled.sql);
    assert!(compiled.sql.contains(
        "LEFT JOIN (SELECT \"__restricted\".\"_idrref\" AS \"_idrref\", \"__restricted\".\"_code\" AS \"_code\", \"__restricted\".\"_date_time\" AS \"_date_time\" FROM \"_reference57\" AS \"__restricted\" WHERE (\"__restricted\".\"_code\" <> 'hidden')) AS \"o\""
    ), "{}", compiled.sql);
}

#[test]
fn wraps_the_extension_union_of_a_restricted_source() {
    let snapshot = with_live_tables(reference_snapshot(), |tables| {
        let mut extension = tables[0].clone();
        extension.name = "_reference53X1".to_owned();
        tables.push(extension);
    });
    let restrictions = [restriction(
        &snapshot,
        "Catalog.OpenSdblMetadataProbe",
        "Code <> \"hidden\"",
    )];
    let compiled = compile_both(
        &snapshot,
        "ВЫБРАТЬ РАЗРЕШЕННЫЕ p.Code ИЗ Catalog.OpenSdblMetadataProbe КАК p",
        &CompileOptions::new().restrictions(&restrictions),
    )
    .unwrap();

    assert!(
        compiled.sql.contains(
            "FROM (SELECT \"__restricted\".\"_idrref\" AS \"_idrref\", \"__restricted\".\"_code\" AS \"_code\", \"__restricted\".\"_date_time\" AS \"_date_time\", \"__restricted\".\"_fld54\" AS \"_fld54\" FROM (SELECT"
        ),
        "{}",
        compiled.sql
    );
    assert!(
        compiled
            .sql
            .contains("FROM \"_reference53\" UNION ALL SELECT")
    );
    assert!(compiled.sql.contains(
        "FROM \"_reference53X1\") AS \"__restricted\" WHERE (\"__restricted\".\"_code\" <> 'hidden')) AS \"p\""
    ));
}
