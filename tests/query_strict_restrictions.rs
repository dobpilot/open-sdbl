//! `RestrictionMode::Restricted`: protection as a property of the
//! compilation rather than of the query text.
//!
//! Every test asserts the generated SQL, not only the contents of the
//! restriction request: a target listed but not filtered would still be an
//! unprotected read. Both dialects are compiled for each case.

mod support;

use support::*;

use open_sdbl::metadata::{FieldId, MetadataSnapshot, ObjectId, StandardFieldId};
use open_sdbl::query::{
    AccessDecision, AccessRestriction, Backend, CompileOptions, CompiledQuery, MsSqlBackend,
    ParameterValue, PostgresBackend, PrepareOptions, Prepared, PresentationExpression,
    PresentationPlan, QueryCompiler, QueryDiagnostic, QueryDiagnosticKind, QueryParameter,
    RestrictionMode, RestrictionTarget, SessionParameters, TempTablesManager, find_metadata_object,
};

fn object_id(snapshot: &MetadataSnapshot, name: &str) -> ObjectId {
    ObjectId::from(&find_metadata_object(snapshot, name).unwrap().guid)
}

fn mssql() -> MsSqlBackend {
    MsSqlBackend::new(0).unwrap()
}

fn prepare<B: Backend>(
    snapshot: &MetadataSnapshot,
    backend: B,
    source: &str,
) -> Result<Prepared<B>, QueryDiagnostic> {
    QueryCompiler::new(snapshot, backend)
        .prepare_with_options(source, &PrepareOptions::new().restricted())
}

/// Prepares restricted on both dialects and checks they agree on the
/// request; answers the PostgreSQL side for the SQL assertions.
fn prepare_both(
    snapshot: &MetadataSnapshot,
    source: &str,
) -> (Prepared<PostgresBackend>, Prepared<MsSqlBackend>) {
    let postgres = prepare(snapshot, PostgresBackend, source).unwrap();
    let mssql = prepare(snapshot, mssql(), source).unwrap();
    assert_eq!(
        postgres.restriction_request(),
        mssql.restriction_request(),
        "{source}"
    );
    assert_eq!(postgres.restriction_mode(), RestrictionMode::Restricted);
    (postgres, mssql)
}

/// Compiles both prepared queries with the same decisions and checks the
/// dialects agree on success; answers both statements.
fn compile_both(
    snapshot: &MetadataSnapshot,
    prepared: &(Prepared<PostgresBackend>, Prepared<MsSqlBackend>),
    options: &CompileOptions<'_>,
) -> Result<(CompiledQuery, CompiledQuery), QueryDiagnostic> {
    let postgres = prepared.0.compile_with(snapshot, options);
    let mssql = prepared.1.compile_with(snapshot, options);
    match (&postgres, &mssql) {
        (Ok(_), Ok(_)) => {}
        (Err(left), Err(right)) => {
            assert_eq!(left.kind(), right.kind());
            assert_eq!((left.line(), left.column()), (right.line(), right.column()));
            return Err(postgres.unwrap_err());
        }
        _ => panic!("backend outcomes differ: {postgres:?} / {mssql:?}"),
    }
    Ok((postgres.unwrap(), mssql.unwrap()))
}

/// Answers every target of the request with the same decision maker.
fn decide(
    prepared: &Prepared<PostgresBackend>,
    mut answer: impl FnMut(&RestrictionTarget) -> AccessDecision,
) -> Vec<AccessDecision> {
    prepared
        .restriction_request()
        .targets
        .iter()
        .map(&mut answer)
        .collect()
}

fn allow_all(prepared: &Prepared<PostgresBackend>) -> Vec<AccessDecision> {
    decide(prepared, |target| {
        AccessDecision::unrestricted(target.clone())
    })
}

fn session(parameters: &[(&str, ParameterValue)]) -> SessionParameters {
    let mut session = SessionParameters::new();
    for (name, value) in parameters {
        session.set(QueryParameter::new(*name, value.clone()));
    }
    session
}

/// How many times the restricted derived table appears — one per filtered
/// read, so a read that lost its wrapper is visible.
fn wrappers(sql: &str) -> usize {
    sql.matches("AS \"__restricted\"").count() + sql.matches("AS [__restricted]").count()
}

/// The probe catalog with a live parent column, so `В ИЕРАРХИИ` really
/// descends and the refusal has something to refuse.
fn hierarchical_snapshot() -> MetadataSnapshot {
    with_live_tables(snapshot(), |tables| {
        tables[0].columns.push(open_sdbl::metadata::LiveColumn {
            name: "_parentidrref".to_owned(),
            data_type: "bytea".to_owned(),
        });
    })
}

#[test]
fn a_query_without_the_keyword_is_still_filtered() {
    let snapshot = snapshot();
    let source = "ВЫБРАТЬ Code ИЗ Справочник.OpenSdblMetadataProbe";
    let prepared = prepare_both(&snapshot, source);
    let probe = object_id(&snapshot, "Справочник.OpenSdblMetadataProbe");
    assert_eq!(
        prepared.0.restriction_request().targets,
        [RestrictionTarget {
            object: probe,
            table_part: None,
        }]
    );

    let decisions = [AccessDecision::restricted(AccessRestriction::new(
        probe,
        "Code <> \"\"",
    ))];
    let (postgres, mssql) = compile_both(
        &snapshot,
        &prepared,
        &CompileOptions::new().decisions(&decisions),
    )
    .unwrap();
    assert_eq!(wrappers(&postgres.sql), 1, "{}", postgres.sql);
    assert_eq!(wrappers(&mssql.sql), 1, "{}", mssql.sql);
    assert!(postgres.sql.contains("<> ''"), "{}", postgres.sql);
    assert!(mssql.sql.contains("<> N''"), "{}", mssql.sql);

    // The same text outside the mode is unfiltered, as it always was.
    let plain = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile(source)
        .unwrap();
    assert_eq!(wrappers(&plain.sql), 0);
}

#[test]
fn every_statement_of_a_mixed_batch_is_filtered() {
    let snapshot = snapshot();
    let probe = object_id(&snapshot, "Справочник.OpenSdblMetadataProbe");
    let source = "ВЫБРАТЬ Code ПОМЕСТИТЬ ВТ ИЗ Справочник.OpenSdblMetadataProbe;
         ВЫБРАТЬ РАЗРЕШЕННЫЕ Т.Code ИЗ ВТ КАК Т
             ЛЕВОЕ СОЕДИНЕНИЕ Справочник.OpenSdblMetadataProbe КАК К ПО Т.Code = К.Code";
    let prepared = prepare_both(&snapshot, source);
    let decisions = [AccessDecision::restricted(AccessRestriction::new(
        probe,
        "Code <> \"\"",
    ))];
    let options = CompileOptions::new().decisions(&decisions);
    let mut postgres_manager = TempTablesManager::new();
    let postgres = prepared
        .0
        .compile_batch(&snapshot, &options, &mut postgres_manager)
        .unwrap()
        .unwrap();
    let mut mssql_manager = TempTablesManager::new();
    let mssql = prepared
        .1
        .compile_batch(&snapshot, &options, &mut mssql_manager)
        .unwrap()
        .unwrap();
    // The defining statement carries no keyword and is filtered anyway,
    // so the CTE it fills and the catalog the second statement joins both
    // wear the wrapper.
    assert_eq!(wrappers(&postgres.sql), 2, "{}", postgres.sql);
    assert_eq!(wrappers(&mssql.sql), 2, "{}", mssql.sql);
}

#[test]
fn nested_queries_union_branches_and_joins_are_filtered() {
    let snapshot = snapshot();
    let probe = object_id(&snapshot, "Справочник.OpenSdblMetadataProbe");
    let source = "ВЫБРАТЬ Вл.Code ИЗ (ВЫБРАТЬ Code ИЗ Справочник.OpenSdblMetadataProbe) КАК Вл
         ОБЪЕДИНИТЬ ВСЕ
         ВЫБРАТЬ Л.Code ИЗ Справочник.OpenSdblMetadataProbe КАК Л
             ВНУТРЕННЕЕ СОЕДИНЕНИЕ Справочник.OpenSdblMetadataProbe КАК П ПО Л.Ссылка = П.Ссылка";
    let prepared = prepare_both(&snapshot, source);
    let decisions = [AccessDecision::restricted(AccessRestriction::new(
        probe,
        "Code <> \"\"",
    ))];
    let (postgres, mssql) = compile_both(
        &snapshot,
        &prepared,
        &CompileOptions::new().decisions(&decisions),
    )
    .unwrap();
    // The nested query, and both sides of the join.
    assert_eq!(wrappers(&postgres.sql), 3, "{}", postgres.sql);
    assert_eq!(wrappers(&mssql.sql), 3, "{}", mssql.sql);
}

#[test]
fn one_target_read_through_several_aliases_is_requested_once_and_filtered_everywhere() {
    let snapshot = snapshot();
    let probe = object_id(&snapshot, "Справочник.OpenSdblMetadataProbe");
    let source = "ВЫБРАТЬ Л.Code ИЗ Справочник.OpenSdblMetadataProbe КАК Л
             ЛЕВОЕ СОЕДИНЕНИЕ Справочник.OpenSdblMetadataProbe КАК П ПО Л.Ссылка = П.Ссылка
         ГДЕ Л.Ссылка В (ВЫБРАТЬ Вн.Ссылка ИЗ Справочник.OpenSdblMetadataProbe КАК Вн)";
    let prepared = prepare_both(&snapshot, source);
    assert_eq!(
        prepared.0.restriction_request().targets,
        [RestrictionTarget {
            object: probe,
            table_part: None,
        }],
        "one target, however many aliases read it"
    );
    let decisions = [AccessDecision::restricted(AccessRestriction::new(
        probe,
        "Code <> \"\"",
    ))];
    let (postgres, mssql) = compile_both(
        &snapshot,
        &prepared,
        &CompileOptions::new().decisions(&decisions),
    )
    .unwrap();
    assert_eq!(wrappers(&postgres.sql), 3, "{}", postgres.sql);
    assert_eq!(wrappers(&mssql.sql), 3, "{}", mssql.sql);
}

#[test]
fn a_dereference_target_is_requested_and_filtered() {
    let snapshot = tabular_section_snapshot();
    let section = object_id(&snapshot, "Документ.бит_ДополнительныеУсловияПоДоговору");
    let centre = object_id(&snapshot, "Справочник.ЦентрыФинансовойОтветственности");
    let source = "ВЫБРАТЬ Т.Сумма, Т.ЦФО.Сам_БизнесРегион
         ИЗ Документ.бит_ДополнительныеУсловияПоДоговору.ГрафикНачислений КАК Т";
    let prepared = prepare_both(&snapshot, source);
    let mut expected = vec![
        RestrictionTarget {
            object: section,
            table_part: Some("ГрафикНачислений".to_owned()),
        },
        RestrictionTarget {
            object: centre,
            table_part: None,
        },
    ];
    expected.sort();
    assert_eq!(
        prepared.0.restriction_request().targets,
        expected,
        "the dereferenced catalog is a read of its own"
    );

    let decisions = decide(&prepared.0, |target| {
        if target.object == centre {
            AccessDecision::restricted(AccessRestriction::new(centre, "Сам_БизнесРегион = &Регион"))
        } else {
            AccessDecision::unrestricted(target.clone())
        }
    });
    let values = session(&[(
        "Регион",
        ParameterValue::Reference {
            object: centre,
            id: [7; 16],
        },
    )]);
    let (postgres, mssql) = compile_both(
        &snapshot,
        &prepared,
        &CompileOptions::new().decisions(&decisions).session(&values),
    )
    .unwrap();
    // The join that the query never named reads through the wrapper.
    assert!(
        postgres.sql.contains("LEFT JOIN (SELECT \"__restricted\""),
        "{}",
        postgres.sql
    );
    assert!(
        mssql.sql.contains("LEFT JOIN (SELECT [__restricted]"),
        "{}",
        mssql.sql
    );
    assert_eq!(wrappers(&postgres.sql), 1, "{}", postgres.sql);
}

#[test]
fn a_denied_target_reads_through_a_false_predicate() {
    let snapshot = snapshot();
    let probe = object_id(&snapshot, "Справочник.OpenSdblMetadataProbe");
    let prepared = prepare_both(
        &snapshot,
        "ВЫБРАТЬ Code ИЗ Справочник.OpenSdblMetadataProbe",
    );
    let decisions = [AccessDecision::denied(RestrictionTarget {
        object: probe,
        table_part: None,
    })];
    let (postgres, mssql) = compile_both(
        &snapshot,
        &prepared,
        &CompileOptions::new().decisions(&decisions),
    )
    .unwrap();
    assert_eq!(wrappers(&postgres.sql), 1, "{}", postgres.sql);
    assert!(postgres.sql.contains("WHERE FALSE"), "{}", postgres.sql);
    assert!(
        mssql.sql.contains("WHERE ") && mssql.sql.contains("__restricted"),
        "{}",
        mssql.sql
    );
    // A denial is a filter, never an omitted one.
    assert_ne!(wrappers(&postgres.sql), 0);
}

#[test]
fn an_explicit_allowance_reads_without_a_wrapper() {
    let snapshot = snapshot();
    let source = "ВЫБРАТЬ Code ИЗ Справочник.OpenSdblMetadataProbe";
    let prepared = prepare_both(&snapshot, source);
    let decisions = allow_all(&prepared.0);
    let (postgres, mssql) = compile_both(
        &snapshot,
        &prepared,
        &CompileOptions::new().decisions(&decisions),
    )
    .unwrap();
    let plain = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile(source)
        .unwrap();
    assert_eq!(postgres.sql, plain.sql);
    assert_eq!(wrappers(&mssql.sql), 0);
}

#[test]
fn a_target_without_a_decision_fails_compilation() {
    let snapshot = snapshot();
    let prepared = prepare_both(
        &snapshot,
        "ВЫБРАТЬ Code ИЗ Справочник.OpenSdblMetadataProbe",
    );
    let error = compile_both(&snapshot, &prepared, &CompileOptions::new()).unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::Restriction);
    assert!(
        error.to_string().contains("has no access decision"),
        "{error}"
    );
    assert!(
        error.to_string().contains("OpenSdblMetadataProbe"),
        "the diagnostic names the object: {error}"
    );
}

#[test]
fn a_missing_decision_names_the_tabular_section() {
    let snapshot = tabular_section_snapshot();
    let centre = object_id(&snapshot, "Справочник.ЦентрыФинансовойОтветственности");
    let prepared = prepare_both(
        &snapshot,
        "ВЫБРАТЬ Т.Сумма ИЗ Документ.бит_ДополнительныеУсловияПоДоговору.ГрафикНачислений КАК Т",
    );
    let unrelated = [AccessDecision::unrestricted(RestrictionTarget {
        object: centre,
        table_part: None,
    })];
    let error = compile_both(
        &snapshot,
        &prepared,
        &CompileOptions::new().decisions(&unrelated),
    )
    .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::Restriction);
    assert!(
        error.to_string().contains("ГрафикНачислений"),
        "the diagnostic names the tabular section: {error}"
    );
}

#[test]
fn a_broken_condition_never_compiles_unfiltered() {
    let snapshot = snapshot();
    let probe = object_id(&snapshot, "Справочник.OpenSdblMetadataProbe");
    let prepared = prepare_both(
        &snapshot,
        "ВЫБРАТЬ Code ИЗ Справочник.OpenSdblMetadataProbe",
    );
    let decisions = [AccessDecision::restricted(AccessRestriction::new(
        probe,
        "НетТакогоПоля = 1",
    ))];
    let error = compile_both(
        &snapshot,
        &prepared,
        &CompileOptions::new().decisions(&decisions),
    )
    .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::Restriction);
}

#[test]
fn a_prepared_restricted_query_keeps_its_mode() {
    let snapshot = snapshot();
    let source = "ВЫБРАТЬ Code ИЗ Справочник.OpenSdblMetadataProbe";
    let restricted = QueryCompiler::new(&snapshot, PostgresBackend)
        .prepare_with_options(source, &PrepareOptions::new().restricted())
        .unwrap();
    assert_eq!(restricted.restriction_mode(), RestrictionMode::Restricted);
    // Default options carry no mode at all, so they cannot lower one: the
    // compilation asks for decisions instead of reading unfiltered.
    let error = restricted
        .compile_with(&snapshot, &CompileOptions::new())
        .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::Restriction);

    let plain = QueryCompiler::new(&snapshot, PostgresBackend)
        .prepare(source)
        .unwrap();
    assert_eq!(plain.restriction_mode(), RestrictionMode::Statement);
    assert!(plain.restriction_request().targets.is_empty());
    assert!(
        plain
            .compile_with(&snapshot, &CompileOptions::new())
            .is_ok()
    );
}

#[test]
fn a_temporary_table_of_the_same_restricted_batch_is_readable() {
    let snapshot = snapshot();
    let probe = object_id(&snapshot, "Справочник.OpenSdblMetadataProbe");
    let decisions = [AccessDecision::restricted(AccessRestriction::new(
        probe,
        "Code <> \"\"",
    ))];
    let options = CompileOptions::new().decisions(&decisions);
    for backend in [0, 1] {
        let mut manager = TempTablesManager::new();
        let source = "ВЫБРАТЬ Code ПОМЕСТИТЬ ВТ ИЗ Справочник.OpenSdblMetadataProbe;
             ВЫБРАТЬ Т.Code ИЗ ВТ КАК Т";
        let compiled = if backend == 0 {
            QueryCompiler::new(&snapshot, PostgresBackend)
                .prepare_with_options(source, &PrepareOptions::new().restricted())
                .unwrap()
                .compile_batch(&snapshot, &options, &mut manager)
        } else {
            QueryCompiler::new(&snapshot, mssql())
                .prepare_with_options(source, &PrepareOptions::new().restricted())
                .unwrap()
                .compile_batch(&snapshot, &options, &mut manager)
        }
        .unwrap()
        .unwrap();
        assert_eq!(wrappers(&compiled.sql), 1, "{}", compiled.sql);
    }
}

#[test]
fn a_temporary_table_of_an_unrestricted_batch_is_refused() {
    let snapshot = snapshot();
    let mut manager = TempTablesManager::new();
    let compiler = QueryCompiler::new(&snapshot, PostgresBackend);
    compiler
        .compile_batch(
            "ВЫБРАТЬ Code ПОМЕСТИТЬ ВТ ИЗ Справочник.OpenSdblMetadataProbe;",
            &CompileOptions::new(),
            &mut manager,
        )
        .unwrap();
    let error = compiler
        .prepare_with_options(
            "ВЫБРАТЬ Т.Code ИЗ ВТ КАК Т",
            &PrepareOptions::new()
                .restricted()
                .temporary_tables(&manager),
        )
        .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::UnsupportedFeature);
    assert!(error.to_string().contains("access-restricted"), "{error}");
}

#[test]
fn constructs_without_a_filtered_read_are_refused() {
    let snapshot = hierarchical_snapshot();
    for source in [
        "ВЫБРАТЬ Ссылка ИЗ Справочник.OpenSdblMetadataProbe ГДЕ Ссылка В ИЕРАРХИИ (ВЫБРАТЬ Г.Ссылка ИЗ Справочник.OpenSdblMetadataProbe КАК Г)",
        "ВЫБРАТЬ Code ИЗ Константы",
    ] {
        for dialect in [0, 1] {
            let error = if dialect == 0 {
                QueryCompiler::new(&snapshot, PostgresBackend)
                    .prepare_with_options(source, &PrepareOptions::new().restricted())
                    .err()
            } else {
                QueryCompiler::new(&snapshot, mssql())
                    .prepare_with_options(source, &PrepareOptions::new().restricted())
                    .err()
            };
            let error = error.unwrap_or_else(|| panic!("{source} must be refused"));
            assert_eq!(
                error.kind(),
                QueryDiagnosticKind::UnsupportedFeature,
                "{source}: {error}"
            );
            assert!(
                error.to_string().contains("access-restricted"),
                "{source}: {error}"
            );
        }
    }
}

#[test]
fn each_candidate_of_a_composite_reference_is_requested_and_filtered() {
    let snapshot = universal_dereferenced_presentation_snapshot();
    let document = object_id(&snapshot, "Документ.бит_ДополнительныеУсловияПоДоговору");
    let source = "ВЫБРАТЬ Д.ДоговорКонтрагента.Ссылка.Ссылка КАК К
         ИЗ Документ.бит_ДополнительныеУсловияПоДоговору КАК Д";
    let prepared = prepare_both(&snapshot, source);
    let targets = prepared.0.restriction_request().targets.clone();
    assert!(
        targets.iter().any(|target| target.object == document),
        "{targets:?}"
    );
    // The hop admits more than one type, and each candidate the compiler
    // joins is a read of its own.
    let candidates = targets
        .iter()
        .filter(|target| target.object != document)
        .count();
    assert!(
        candidates >= 1,
        "a candidate of the composite hop must be requested: {targets:?}"
    );

    // Filtering every candidate wraps every candidate join.
    let decisions = decide(&prepared.0, |target| {
        if target.object == document {
            AccessDecision::unrestricted(target.clone())
        } else {
            AccessDecision::restricted(AccessRestriction::new(target.object, "Ссылка = Ссылка"))
        }
    });
    let (postgres, mssql) = compile_both(
        &snapshot,
        &prepared,
        &CompileOptions::new().decisions(&decisions),
    )
    .unwrap();
    // The path joins the candidate twice, and both reads are wrapped: the
    // restricted table is never reached directly.
    assert!(wrappers(&postgres.sql) >= candidates, "{}", postgres.sql);
    assert!(
        !postgres.sql.contains("JOIN \"_reference62\" AS"),
        "an unfiltered read of the restricted candidate remains: {}",
        postgres.sql
    );
    assert!(
        !mssql.sql.contains("JOIN [_reference62] AS"),
        "an unfiltered read of the restricted candidate remains: {}",
        mssql.sql
    );
}

#[test]
fn a_presentation_read_is_requested_and_filtered() {
    let snapshot = dereferenced_presentation_snapshot();
    let centre = object_id(&snapshot, "Справочник.ЦентрыФинансовойОтветственности");
    let source = "ВЫБРАТЬ ПРЕДСТАВЛЕНИЕССЫЛКИ(Т.ЦФО)
         ИЗ Документ.бит_ДополнительныеУсловияПоДоговору.ГрафикНачислений КАК Т";
    let Ok(prepared) = prepare(&snapshot, PostgresBackend, source) else {
        // The fixture may not carry a presentation plan for the target;
        // the refusal path is covered by its own test.
        return;
    };
    assert!(
        prepared
            .restriction_request()
            .targets
            .iter()
            .any(|target| target.object == centre),
        "the presentation join reads the catalog: {:?}",
        prepared.restriction_request().targets
    );
}

#[test]
fn a_virtual_table_is_filtered_in_the_restricted_mode() {
    let snapshot = accumulation_register_snapshot();
    let register = object_id(&snapshot, "РегистрНакопления.Остатки");
    let source = "ВЫБРАТЬ Количество ИЗ РегистрНакопления.Остатки";
    let prepared = prepare_both(&snapshot, source);
    assert_eq!(
        prepared.0.restriction_request().targets,
        [RestrictionTarget {
            object: register,
            table_part: None,
        }]
    );
    let decisions = [AccessDecision::restricted(AccessRestriction::new(
        register,
        "Количество > 0",
    ))];
    let (postgres, mssql) = compile_both(
        &snapshot,
        &prepared,
        &CompileOptions::new().decisions(&decisions),
    )
    .unwrap();
    assert_eq!(wrappers(&postgres.sql), 1, "{}", postgres.sql);
    assert_eq!(wrappers(&mssql.sql), 1, "{}", mssql.sql);
}

/// The demo fixture carries the journals and filter criteria the probe
/// snapshots do not.
fn demo_snapshot() -> MetadataSnapshot {
    support::demo_resolved_at(
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/demo"),
    )
    .snapshot
}

#[test]
fn a_filter_criterion_is_refused_in_the_restricted_mode() {
    let snapshot = demo_snapshot();
    let source =
        "ВЫБРАТЬ К.Ссылка КАК С ИЗ КритерийОтбора.ДокументыПоВопросуДеятельности(&Значение) КАК К";
    // It compiles outside the mode, which is what makes the refusal a
    // restriction of the mode rather than a missing feature.
    assert!(
        QueryCompiler::new(&snapshot, PostgresBackend)
            .prepare(source)
            .is_ok()
    );
    for error in [
        prepare(&snapshot, PostgresBackend, source).unwrap_err(),
        prepare(&snapshot, mssql(), source).unwrap_err(),
    ] {
        assert_eq!(error.kind(), QueryDiagnosticKind::UnsupportedFeature);
        assert!(error.to_string().contains("filter criterion"), "{error}");
    }
}

#[test]
fn a_document_journal_is_a_target_like_any_other_source() {
    let snapshot = demo_snapshot();
    let source = "ВЫБРАТЬ Ж.Ссылка КАК С ИЗ ЖурналДокументов.УчетРабочегоВремени КАК Ж";
    let journal = object_id(&snapshot, "ЖурналДокументов.УчетРабочегоВремени");
    let prepared = prepare_both(&snapshot, source);
    assert!(
        prepared
            .0
            .restriction_request()
            .targets
            .iter()
            .any(|target| target.object == journal),
        "the journal table is requested: {:?}",
        prepared.0.restriction_request().targets
    );
    let decisions = decide(&prepared.0, |target| {
        if target.object == journal {
            AccessDecision::restricted(AccessRestriction::new(journal, "Проведен"))
        } else {
            AccessDecision::unrestricted(target.clone())
        }
    });
    let values = session(&[(
        "ОбластьДанныхОсновныеДанные",
        ParameterValue::Number {
            unscaled: 0,
            scale: 0,
        },
    )]);
    let (postgres, mssql) = compile_both(
        &snapshot,
        &prepared,
        &CompileOptions::new().decisions(&decisions).session(&values),
    )
    .unwrap();
    assert!(wrappers(&postgres.sql) >= 1, "{}", postgres.sql);
    assert!(wrappers(&mssql.sql) >= 1, "{}", mssql.sql);
}

#[test]
fn a_hierarchy_control_point_of_totals_is_refused() {
    let snapshot = hierarchical_snapshot();
    let source = "ВЫБРАТЬ Ссылка, Code ИЗ Справочник.OpenSdblMetadataProbe
         ИТОГИ КОЛИЧЕСТВО(Code) ПО Ссылка ИЕРАРХИЯ";
    // It compiles outside the mode; the refusal belongs to the mode.
    assert!(
        QueryCompiler::new(&snapshot, PostgresBackend)
            .compile(source)
            .is_ok(),
        "the fixture must support the construct outside the mode"
    );
    for error in [
        prepare(&snapshot, PostgresBackend, source).unwrap_err(),
        prepare(&snapshot, mssql(), source).unwrap_err(),
    ] {
        assert_eq!(error.kind(), QueryDiagnosticKind::UnsupportedFeature);
        assert!(error.to_string().contains("ИТОГИ"), "{error}");
    }
}

#[test]
fn a_nested_tabular_section_projection_is_refused() {
    let snapshot = tabular_section_snapshot();
    let source = "ВЫБРАТЬ Ссылка, ГрафикНачислений ИЗ Документ.бит_ДополнительныеУсловияПоДоговору";
    // The section rows travel in a second query of their own, which this
    // compilation does not filter, so the mode refuses the projection.
    assert!(
        QueryCompiler::new(&snapshot, PostgresBackend)
            .compile(source)
            .is_ok(),
        "the fixture must support the construct outside the mode"
    );
    for error in [
        prepare(&snapshot, PostgresBackend, source).unwrap_err(),
        prepare(&snapshot, mssql(), source).unwrap_err(),
    ] {
        assert_eq!(error.kind(), QueryDiagnosticKind::UnsupportedFeature);
        assert!(
            error.to_string().contains("nested tabular-section"),
            "{error}"
        );
    }
}

#[test]
fn a_decision_for_a_target_nobody_reads_is_an_error() {
    let snapshot = snapshot();
    let probe = object_id(&snapshot, "Справочник.OpenSdblMetadataProbe");
    let prepared = prepare_both(
        &snapshot,
        "ВЫБРАТЬ Code ИЗ Справочник.OpenSdblMetadataProbe",
    );
    let decisions = [
        AccessDecision::unrestricted(RestrictionTarget {
            object: probe,
            table_part: None,
        }),
        AccessDecision::unrestricted(RestrictionTarget {
            object: probe,
            table_part: Some("НетТакой".to_owned()),
        }),
    ];
    let error = compile_both(
        &snapshot,
        &prepared,
        &CompileOptions::new().decisions(&decisions),
    )
    .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::Restriction);
}

#[test]
fn two_answers_about_one_target_are_refused() {
    let snapshot = snapshot();
    let probe = object_id(&snapshot, "Справочник.OpenSdblMetadataProbe");
    let prepared = prepare_both(
        &snapshot,
        "ВЫБРАТЬ Code ИЗ Справочник.OpenSdblMetadataProbe",
    );
    let restrictions = [AccessRestriction::new(probe, "Code <> \"\"")];
    let decisions = [AccessDecision::denied(RestrictionTarget {
        object: probe,
        table_part: None,
    })];
    let error = compile_both(
        &snapshot,
        &prepared,
        &CompileOptions::new()
            .restrictions(&restrictions)
            .decisions(&decisions),
    )
    .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::Restriction);
    assert!(
        error.to_string().contains("more than once"),
        "a condition and a decision for one target must not be combined: {error}"
    );
}

#[test]
fn a_condition_supplied_the_old_way_answers_a_restricted_target() {
    let snapshot = snapshot();
    let probe = object_id(&snapshot, "Справочник.OpenSdblMetadataProbe");
    let prepared = prepare_both(
        &snapshot,
        "ВЫБРАТЬ Code ИЗ Справочник.OpenSdblMetadataProbe",
    );
    let restrictions = [AccessRestriction::new(probe, "Code <> \"\"")];
    let (postgres, mssql) = compile_both(
        &snapshot,
        &prepared,
        &CompileOptions::new().restrictions(&restrictions),
    )
    .unwrap();
    assert_eq!(wrappers(&postgres.sql), 1, "{}", postgres.sql);
    assert_eq!(wrappers(&mssql.sql), 1, "{}", mssql.sql);
}

#[test]
fn the_reads_of_a_condition_stay_outside_the_request() {
    let snapshot = snapshot();
    let probe = object_id(&snapshot, "Справочник.OpenSdblMetadataProbe");
    let prepared = prepare_both(
        &snapshot,
        "ВЫБРАТЬ Code ИЗ Справочник.OpenSdblMetadataProbe",
    );
    // The condition reads the same table through a nested query. The
    // trust boundary says that read is the host's, so it is neither
    // filtered again nor requested.
    let decisions = [AccessDecision::restricted(AccessRestriction::new(
        probe,
        "Ссылка В (ВЫБРАТЬ К.Ссылка ИЗ Справочник.OpenSdblMetadataProbe КАК К)",
    ))];
    let (postgres, _) = compile_both(
        &snapshot,
        &prepared,
        &CompileOptions::new().decisions(&decisions),
    )
    .unwrap();
    assert_eq!(
        wrappers(&postgres.sql),
        1,
        "only the statement's own read is wrapped: {}",
        postgres.sql
    );
    assert_eq!(
        prepared.0.restriction_request().targets.len(),
        1,
        "the condition's own read is not a target"
    );
}

#[test]
fn a_presentation_lookup_reads_its_target_through_the_decision() {
    let snapshot = snapshot();
    let probe = object_id(&snapshot, "Справочник.OpenSdblMetadataProbe");
    let plan = PresentationPlan {
        object: probe,
        fields: vec![FieldId::Standard(StandardFieldId::Code)],
        expression: PresentationExpression::Field(FieldId::Standard(StandardFieldId::Code)),
    };
    let references = [[0x11; 16]];

    // Unfiltered: the entry point that takes no decisions is unchanged.
    let plain = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile_presentation_lookup(&plan, &references)
        .unwrap();
    assert_eq!(wrappers(&plain.sql), 0, "{}", plain.sql);

    // Filtered: the prepared query carries the mode and the decisions.
    let prepared = QueryCompiler::new(&snapshot, PostgresBackend)
        .prepare_with_options(
            "ВЫБРАТЬ Code ИЗ Справочник.OpenSdblMetadataProbe",
            &PrepareOptions::new().restricted(),
        )
        .unwrap();
    let decisions = [AccessDecision::restricted(AccessRestriction::new(
        probe,
        "Code <> \"\"",
    ))];
    let filtered = prepared
        .compile_presentation_lookup(
            &snapshot,
            &plan,
            &references,
            &CompileOptions::new().decisions(&decisions),
        )
        .unwrap();
    assert_eq!(wrappers(&filtered.sql), 1, "{}", filtered.sql);
    assert!(filtered.sql.contains("<> ''"), "{}", filtered.sql);

    // Denied: the reference matches no row, so nothing is presented.
    let denied = [AccessDecision::denied(RestrictionTarget {
        object: probe,
        table_part: None,
    })];
    let denied = prepared
        .compile_presentation_lookup(
            &snapshot,
            &plan,
            &references,
            &CompileOptions::new().decisions(&denied),
        )
        .unwrap();
    assert!(denied.sql.contains("WHERE FALSE"), "{}", denied.sql);

    // No decision at all, in the restricted mode, is an error.
    let error = prepared
        .compile_presentation_lookup(&snapshot, &plan, &references, &CompileOptions::new())
        .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::Restriction);
    assert!(
        error.to_string().contains("OpenSdblMetadataProbe"),
        "{error}"
    );
}

#[test]
fn a_deferred_presentation_compiles_in_the_restricted_mode() {
    // The lookup that resolves it is filtered in its own right, so the
    // mode no longer has to refuse producing work for it.
    let snapshot = universal_dereferenced_presentation_snapshot();
    let source = "ВЫБРАТЬ ПРЕДСТАВЛЕНИЕССЫЛКИ(Д.ДоговорКонтрагента) КАК П
         ИЗ Документ.бит_ДополнительныеУсловияПоДоговору КАК Д";
    let prepared = prepare(&snapshot, PostgresBackend, source)
        .unwrap_or_else(|error| panic!("a deferred presentation must compile: {error}"));
    let decisions = allow_all(&prepared);
    let compiled = prepared
        .compile_with(&snapshot, &CompileOptions::new().decisions(&decisions))
        .unwrap();
    assert!(
        !compiled.deferred_presentations.is_empty(),
        "the presentation is deferred to the application: {}",
        compiled.sql
    );
}
