//! Data-separator predicates on every separated table a statement reads.
//!
//! The snapshot comes from the probe base captured under
//! `tests/fixtures/separators`: catalog `Товары` (`Артикул`, `Поставщик`),
//! constant `ОсновнойТовар`, and two separators bound to the session
//! parameters `ЗначениеРазделителя` and `ИспользованиеРазделителя`:
//! `РазделительНезависимо` (`_Fld56`, `Независимо`) and
//! `РазделительСовместно` (`_Fld57`, `Независимо и совместно`).

mod support;

use support::*;

use open_sdbl::metadata::{
    DataSeparationSettings, LiveColumn, LiveTable, MetadataSnapshot, ResolutionFinding,
    SeparatedDataUse, parse_config_descriptors, resolve_metadata,
};
use open_sdbl::query::{
    Backend, CompileOptions, CompiledQuery, MsSqlBackend, ParameterValue, PostgresBackend,
    QueryCompiler, QueryDiagnostic, QueryDiagnosticKind, QueryParameter, SessionParameters,
};

const INDEPENDENT: &str = "a1b2c3d4-0003-4000-8000-000000000003";
const VALUE_PARAMETER: &str = "a1b2c3d4-0001-4000-8000-000000000001";
const USE_PARAMETER: &str = "a1b2c3d4-0002-4000-8000-000000000002";

fn mssql() -> MsSqlBackend {
    MsSqlBackend::new(2000).unwrap()
}

fn compile<B: Backend>(
    snapshot: &MetadataSnapshot,
    backend: B,
    source: &str,
    options: &CompileOptions<'_>,
) -> Result<CompiledQuery, QueryDiagnostic> {
    QueryCompiler::new(snapshot, backend).compile_with(source, options)
}

fn postgres_sql(snapshot: &MetadataSnapshot, source: &str, session: &SessionParameters) -> String {
    compile(
        snapshot,
        PostgresBackend,
        source,
        &CompileOptions::new().session(session),
    )
    .unwrap_or_else(|error| panic!("{source}: {error}"))
    .sql
}

fn session(parameters: &[(&str, ParameterValue)]) -> SessionParameters {
    let mut session = SessionParameters::new();
    for (name, value) in parameters {
        session.set(QueryParameter::new(*name, value.clone()));
    }
    session
}

fn number(value: i128) -> ParameterValue {
    ParameterValue::Number {
        unscaled: value,
        scale: 0,
    }
}

fn area(value: i128) -> SessionParameters {
    session(&[("ЗначениеРазделителя", number(value))])
}

/// The probe snapshot with `РазделительНезависимо` switched to
/// `Независимо и совместно` and unbound, so it falls back to its own name
/// and to the empty value, while `РазделительСовместно` keeps its bindings.
fn shared_snapshot() -> MetadataSnapshot {
    with_descriptors(separators_snapshot(), |descriptors| {
        let independent = descriptors
            .iter_mut()
            .find(|descriptor| descriptor.object_guid == guid(INDEPENDENT))
            .unwrap();
        independent.separation = Some(DataSeparationSettings {
            mode: SeparatedDataUse::IndependentAndShared,
            value_parameter: None,
            use_parameter: None,
        });
    })
}

#[test]
fn resolves_separation_settings_from_the_probe_resources() {
    let resolved = separators_resolved();
    assert!(
        !resolved
            .report
            .findings()
            .iter()
            .any(|finding| matches!(finding, ResolutionFinding::SeparatorSettingsMissing { .. })),
        "{:?}",
        resolved.report.findings()
    );
    let separators = resolved.snapshot.separators().collect::<Vec<_>>();
    assert_eq!(separators.len(), 2);
    let independent = &separators[0];
    assert_eq!(independent.name.as_deref(), Some("РазделительНезависимо"));
    assert_eq!(independent.physical_name, "_Fld56");
    let separation = independent.separation.as_ref().unwrap();
    assert_eq!(separation.mode, SeparatedDataUse::Independent);
    assert_eq!(
        separation.value_parameter.as_deref(),
        Some("ЗначениеРазделителя")
    );
    assert_eq!(
        separation.use_parameter.as_deref(),
        Some("ИспользованиеРазделителя")
    );
    let shared = &separators[1];
    assert_eq!(shared.physical_name, "_Fld57");
    assert_eq!(
        shared.separation.as_ref().unwrap().mode,
        SeparatedDataUse::IndependentAndShared
    );
}

#[test]
fn reports_a_separator_whose_settings_are_missing() {
    let base = separators_snapshot();
    let descriptors = base
        .descriptors()
        .iter()
        .cloned()
        .map(|mut descriptor| {
            if descriptor.object_guid == guid(INDEPENDENT) {
                descriptor.separation = None;
            }
            descriptor
        })
        .collect();
    let resolved = resolve_metadata(
        base.db_names().clone(),
        descriptors,
        base.schema().clone(),
        base.live_tables().to_vec(),
    );
    assert!(
        resolved
            .report
            .findings()
            .contains(&ResolutionFinding::SeparatorSettingsMissing {
                guid: guid(INDEPENDENT),
                column: "_Fld56".to_owned(),
            })
    );
    let field = resolved
        .snapshot
        .separators()
        .find(|field| field.physical_name == "_Fld56")
        .unwrap();
    assert!(field.data_separator);
    assert!(field.separation.is_none());
}

fn common_attribute_resource(tail: &str) -> Vec<u8> {
    let text = format!(
        "{{1,\n{{5,\n{{27,\n{{2,\n{{3,\n{{1,0,{INDEPENDENT}}},\"Разделитель\",\n{{1,\"ru\",\"Разделитель\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},\n{{\"Pattern\",\n{{\"N\",7,0,1}}\n}}\n}},0,\n{{0}},\n{{\"U\"}},0,0,0}},\n{tail}}},0}}"
    );
    stored_deflate(text.as_bytes())
}

#[test]
fn projects_separation_settings_from_the_class_list_tail() {
    let resource = common_attribute_resource(&format!(
        "{{3,0}},0,1,0,0,\n{{1,{VALUE_PARAMETER}}},\n{{1,{USE_PARAMETER}}},\n{{1,00000000-0000-0000-0000-000000000000}},1,1,1,0,1"
    ));
    let descriptors = parse_config_descriptors(INDEPENDENT, &resource).unwrap();
    assert_eq!(
        descriptors[0].separation,
        Some(DataSeparationSettings {
            mode: SeparatedDataUse::IndependentAndShared,
            value_parameter: Some(guid(VALUE_PARAMETER)),
            use_parameter: Some(guid(USE_PARAMETER)),
        })
    );

    let truncated = common_attribute_resource(&format!(
        "{{3,0}},0,1,0,0,\n{{1,{VALUE_PARAMETER}}},\n{{1,{USE_PARAMETER}}}"
    ));
    let descriptors = parse_config_descriptors(INDEPENDENT, &truncated).unwrap();
    assert_eq!(descriptors[0].name, "Разделитель");
    assert_eq!(descriptors[0].separation, None);

    let unknown_mode = common_attribute_resource(&format!(
        "{{3,0}},0,1,0,0,\n{{1,{VALUE_PARAMETER}}},\n{{1,{USE_PARAMETER}}},\n{{1,00000000-0000-0000-0000-000000000000}},1,1,7,0,1"
    ));
    let descriptors = parse_config_descriptors(INDEPENDENT, &unknown_mode).unwrap();
    assert_eq!(descriptors[0].separation, None);
}

#[test]
fn filters_a_plain_source_by_the_session_value_before_the_query_filter() {
    let snapshot = separators_snapshot();
    let sql = postgres_sql(
        &snapshot,
        "ВЫБРАТЬ Т.Ссылка ИЗ Справочник.Товары КАК Т ГДЕ Т.Артикул = \"x\"",
        &area(7),
    );
    assert_eq!(
        sql,
        "SELECT \"Т\".\"_idrref\" AS \"ID\" FROM \"_reference53\" AS \"Т\" WHERE \"Т\".\"_fld56\" = 7 AND \"Т\".\"_fld57\" = 7 AND (\"Т\".\"_fld54\" = 'x')"
    );
    let mssql = compile(
        &snapshot,
        mssql(),
        "ВЫБРАТЬ Т.Ссылка ИЗ Справочник.Товары КАК Т",
        &CompileOptions::new().session(&area(7)),
    )
    .unwrap()
    .sql;
    assert_eq!(
        mssql,
        "SELECT [Т].[_idrref] AS [ID] FROM [_reference53] AS [Т] WHERE [Т].[_fld56] = 7 AND [Т].[_fld57] = 7"
    );
}

#[test]
fn an_independent_separator_without_a_value_is_a_parameter_diagnostic() {
    let snapshot = separators_snapshot();
    for backend in [
        compile(
            &snapshot,
            PostgresBackend,
            "ВЫБРАТЬ Т.Ссылка ИЗ Справочник.Товары КАК Т",
            &CompileOptions::new(),
        ),
        compile(
            &snapshot,
            mssql(),
            "ВЫБРАТЬ Т.Ссылка ИЗ Справочник.Товары КАК Т",
            &CompileOptions::new(),
        ),
    ] {
        let error = backend.unwrap_err();
        assert_eq!(error.kind(), QueryDiagnosticKind::Parameter);
        assert_eq!((error.line(), error.column()), (1, 32));
        assert_eq!(
            error.to_string(),
            "1:32: data separator \"РазделительНезависимо\" requires session parameter \"ЗначениеРазделителя\""
        );
    }
}

#[test]
fn a_shared_separator_defaults_to_the_empty_value_and_falls_back_to_its_name() {
    let snapshot = shared_snapshot();
    let source = "ВЫБРАТЬ Т.Ссылка ИЗ Справочник.Товары КАК Т";
    assert_eq!(
        postgres_sql(&snapshot, source, &SessionParameters::new()),
        "SELECT \"Т\".\"_idrref\" AS \"ID\" FROM \"_reference53\" AS \"Т\" WHERE \"Т\".\"_fld56\" = 0 AND \"Т\".\"_fld57\" = 0"
    );
    assert_eq!(
        postgres_sql(&snapshot, source, &area(7)),
        "SELECT \"Т\".\"_idrref\" AS \"ID\" FROM \"_reference53\" AS \"Т\" WHERE \"Т\".\"_fld56\" = 0 AND \"Т\".\"_fld57\" = 7"
    );
    assert_eq!(
        postgres_sql(
            &snapshot,
            source,
            &session(&[
                ("РазделительНезависимо", number(3)),
                ("ЗначениеРазделителя", number(7)),
            ])
        ),
        "SELECT \"Т\".\"_idrref\" AS \"ID\" FROM \"_reference53\" AS \"Т\" WHERE \"Т\".\"_fld56\" = 3 AND \"Т\".\"_fld57\" = 7"
    );
}

#[test]
fn the_use_flag_disables_the_predicate_and_query_parameters_never_supply_it() {
    let snapshot = separators_snapshot();
    let source = "ВЫБРАТЬ Т.Ссылка ИЗ Справочник.Товары КАК Т";
    let disabled = session(&[("ИспользованиеРазделителя", ParameterValue::Boolean(false))]);
    assert_eq!(
        postgres_sql(&snapshot, source, &disabled),
        "SELECT \"Т\".\"_idrref\" AS \"ID\" FROM \"_reference53\" AS \"Т\""
    );
    let query_value = [QueryParameter::new("ЗначениеРазделителя", number(7))];
    let error = compile(
        &snapshot,
        PostgresBackend,
        source,
        &CompileOptions::new().parameters(&query_value),
    )
    .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::Parameter);
}

#[test]
fn places_predicates_by_join_shape() {
    let snapshot = separators_snapshot();
    let left = postgres_sql(
        &snapshot,
        "ВЫБРАТЬ Т.Ссылка, П.Артикул, Т.Поставщик.Артикул КАК А ИЗ Справочник.Товары КАК Т ЛЕВОЕ СОЕДИНЕНИЕ Справочник.Товары КАК П ПО Т.Поставщик = П.Ссылка ГДЕ Т.Артикул = \"x\"",
        &area(7),
    );
    assert_eq!(
        left,
        "SELECT \"Т\".\"_idrref\" AS \"ID\", \"П\".\"_fld54\"::text AS \"Артикул\", \"__left_ref1\".\"_fld54\"::text AS \"А\" FROM \"_reference53\" AS \"Т\" LEFT JOIN \"_reference53\" AS \"П\" ON \"Т\".\"_fld55rref\" = \"П\".\"_idrref\" AND \"П\".\"_fld56\" = 7 AND \"П\".\"_fld57\" = 7 LEFT JOIN \"_reference53\" AS \"__left_ref1\" ON \"Т\".\"_fld55rref\" = \"__left_ref1\".\"_idrref\" AND \"__left_ref1\".\"_fld56\" = 7 AND \"__left_ref1\".\"_fld57\" = 7 WHERE \"Т\".\"_fld56\" = 7 AND \"Т\".\"_fld57\" = 7 AND (\"Т\".\"_fld54\" = 'x')"
    );

    let right = postgres_sql(
        &snapshot,
        "ВЫБРАТЬ Т.Ссылка ИЗ Справочник.Товары КАК Т ПРАВОЕ СОЕДИНЕНИЕ Справочник.Товары КАК П ПО Т.Поставщик = П.Ссылка",
        &area(7),
    );
    assert_eq!(
        right,
        "SELECT \"Т\".\"_idrref\" AS \"ID\" FROM \"_reference53\" AS \"Т\" RIGHT JOIN \"_reference53\" AS \"П\" ON \"Т\".\"_fld55rref\" = \"П\".\"_idrref\" AND \"Т\".\"_fld56\" = 7 AND \"Т\".\"_fld57\" = 7 WHERE \"П\".\"_fld56\" = 7 AND \"П\".\"_fld57\" = 7"
    );

    let full = postgres_sql(
        &snapshot,
        "ВЫБРАТЬ Т.Ссылка ИЗ Справочник.Товары КАК Т ПОЛНОЕ СОЕДИНЕНИЕ Справочник.Товары КАК П ПО Т.Поставщик = П.Ссылка",
        &area(7),
    );
    assert_eq!(
        full,
        "SELECT * FROM ((SELECT \"Т\".\"_idrref\" AS \"ID\" FROM \"_reference53\" AS \"Т\" LEFT JOIN \"_reference53\" AS \"П\" ON \"Т\".\"_fld55rref\" = \"П\".\"_idrref\" AND \"П\".\"_fld56\" = 7 AND \"П\".\"_fld57\" = 7 WHERE \"Т\".\"_fld56\" = 7 AND \"Т\".\"_fld57\" = 7) UNION ALL (SELECT \"Т\".\"_idrref\" AS \"ID\" FROM \"_reference53\" AS \"П\" LEFT JOIN \"_reference53\" AS \"Т\" ON \"Т\".\"_fld55rref\" = \"П\".\"_idrref\" AND \"Т\".\"_fld56\" = 7 AND \"Т\".\"_fld57\" = 7 WHERE \"П\".\"_fld56\" = 7 AND \"П\".\"_fld57\" = 7 AND (\"Т\".\"_fld55rref\" IS NULL))) AS \"__full\""
    );
}

#[test]
fn filters_constants_nested_queries_and_restricted_sources() {
    let snapshot = separators_snapshot();
    assert_eq!(
        postgres_sql(
            &snapshot,
            "ВЫБРАТЬ К.ОсновнойТовар ИЗ Константа.ОсновнойТовар КАК К",
            &area(7)
        ),
        "SELECT \"К\".\"_fld62rref\" AS \"ОсновнойТовар\" FROM \"_const61\" AS \"К\" WHERE \"К\".\"_fld56\" = 7 AND \"К\".\"_fld57\" = 7"
    );
    assert_eq!(
        postgres_sql(
            &snapshot,
            "ВЫБРАТЬ Т.Ссылка ИЗ Справочник.Товары КАК Т ГДЕ Т.Ссылка В (ВЫБРАТЬ П.Поставщик ИЗ Справочник.Товары КАК П)",
            &area(7)
        ),
        "SELECT \"Т\".\"_idrref\" AS \"ID\" FROM \"_reference53\" AS \"Т\" WHERE \"Т\".\"_fld56\" = 7 AND \"Т\".\"_fld57\" = 7 AND (\"Т\".\"_idrref\" IN (SELECT \"П\".\"_fld55rref\" AS \"Поставщик\" FROM \"_reference53\" AS \"П\" WHERE \"П\".\"_fld56\" = 7 AND \"П\".\"_fld57\" = 7))"
    );

    let restriction = open_sdbl::query::AccessRestriction::new(
        open_sdbl::metadata::ObjectId::from(
            &open_sdbl::query::find_metadata_object(&snapshot, "Справочник.Товары")
                .unwrap()
                .guid,
        ),
        "Артикул <> \"\"",
    );
    let restrictions = [restriction];
    let restricted = compile(
        &snapshot,
        PostgresBackend,
        "ВЫБРАТЬ РАЗРЕШЕННЫЕ Т.Ссылка ИЗ Справочник.Товары КАК Т",
        &CompileOptions::new()
            .session(&area(7))
            .restrictions(&restrictions),
    )
    .unwrap()
    .sql;
    assert!(
        restricted.contains(
            "AS \"__restricted\" WHERE \"__restricted\".\"_fld56\" = 7 AND \"__restricted\".\"_fld57\" = 7 AND (\"__restricted\".\"_fld54\" <> '')) AS \"Т\""
        ),
        "{restricted}"
    );
    assert!(!restricted.contains("\"Т\".\"_fld56\""), "{restricted}");
}

#[test]
fn extension_branches_are_filtered_individually() {
    let snapshot = with_live_tables(separators_snapshot(), |live_tables| {
        live_tables.push(LiveTable {
            name: "_reference53x1".to_owned(),
            columns: ["_idrref", "_fld54", "_fld55rref", "_fld56"]
                .iter()
                .map(|name| LiveColumn {
                    name: (*name).to_owned(),
                    data_type: "bytea".to_owned(),
                })
                .collect(),
            indexes: Vec::new(),
        });
    });
    let sql = postgres_sql(
        &snapshot,
        "ВЫБРАТЬ Т.Ссылка ИЗ Справочник.Товары КАК Т",
        &area(7),
    );
    assert!(
        sql.contains(
            "FROM \"_reference53\" WHERE \"_reference53\".\"_fld56\" = 7 AND \"_reference53\".\"_fld57\" = 7 UNION ALL "
        ),
        "{sql}"
    );
    assert!(
        sql.contains("FROM \"_reference53x1\" WHERE \"_reference53x1\".\"_fld56\" = 7) AS \"Т\""),
        "{sql}"
    );
    assert!(!sql.contains("\"Т\".\"_fld56\""), "{sql}");
}

#[test]
fn a_snapshot_without_separators_is_unchanged() {
    let snapshot = snapshot();
    let source = "ВЫБРАТЬ Т.Ссылка ИЗ Справочник.OpenSdblMetadataProbe КАК Т";
    assert_eq!(
        postgres_sql(&snapshot, source, &area(7)),
        postgres_sql(&snapshot, source, &SessionParameters::new())
    );
}

/// Adds a separator column `_Fld<number>` (numeric, no Config settings, so
/// `Независимо и совместно` by default) to every table of `base`.
fn with_separator(base: MetadataSnapshot, number: u32) -> MetadataSnapshot {
    let separator = guid("a1b2c3d4-0009-4000-8000-000000000009");
    let entries = base
        .db_names()
        .entries()
        .iter()
        .map(|entry| format!("{{{},\"{}\",{}}}", entry.guid, entry.alias, entry.number))
        .chain([
            format!("{{{separator},\"Fld\",{number}}}"),
            format!("{{{separator},\"DataSeparationUse\",{}}}", number + 1),
        ])
        .collect::<Vec<_>>();
    let serialized = format!("{{{},{}}}", entries.len(), entries.join(","));
    let db_names =
        open_sdbl::metadata::parse_db_names(&stored_deflate(serialized.as_bytes())).unwrap();
    let mut schema = base.schema().clone();
    for table in &mut schema.tables {
        table
            .columns
            .push(schema_column(&format!("Fld{number}"), "N", None));
    }
    let mut live_tables = base.live_tables().to_vec();
    for table in &mut live_tables {
        table.columns.push(LiveColumn {
            name: format!("_fld{number}"),
            data_type: "numeric(7,0)".to_owned(),
        });
    }
    resolve_metadata(db_names, base.descriptors().to_vec(), schema, live_tables).snapshot
}

#[test]
fn filters_register_virtual_tables_inside_their_base_reads() {
    let slice = with_separator(information_register_snapshot(), 999);
    let sql = postgres_sql(
        &slice,
        "ВЫБРАТЬ Период ИЗ РегистрСведений.Prices.СрезПоследних(\"2026-08-30\", ProbeAttribute ЕСТЬ НЕ NULL)",
        &SessionParameters::new(),
    );
    assert!(
        sql.contains(
            "FROM \"_inforg53\" AS \"__slice_base\" WHERE \"__slice_base\".\"_fld999\" = 0 AND (\"__slice_base\".\"_period\" <= '2026-08-30')"
        ),
        "{sql}"
    );

    let balance = with_separator(accumulation_register_snapshot(), 999);
    let historical = postgres_sql(
        &balance,
        "ВЫБРАТЬ КоличествоОстаток ИЗ РегистрНакопления.Остатки.Остатки(\"2026-09-01\")",
        &session(&[("_Fld999", number(4))]),
    );
    assert!(
        historical.contains("\"__totals_base\".\"_period\" = \"__balance_anchor\".\"__period\" AND \"__totals_base\".\"_fld999\" = 4 UNION ALL"),
        "{historical}"
    );
    assert!(
        historical.contains(
            "< \"__balance_anchor\".\"__period\")) AND \"__movement_base\".\"_fld999\" = 4)"
        ),
        "{historical}"
    );
    let current = postgres_sql(
        &balance,
        "ВЫБРАТЬ КоличествоОстаток ИЗ РегистрНакопления.Остатки.Остатки()",
        &SessionParameters::new(),
    );
    assert!(
        current.contains("AS \"__totals_latest\") AND \"__totals_base\".\"_fld999\" = 0 GROUP BY"),
        "{current}"
    );
    let turnovers = postgres_sql(
        &balance,
        "ВЫБРАТЬ КоличествоОборот ИЗ РегистрНакопления.Остатки.Обороты(\"2026-01-01\", \"2026-09-01\")",
        &SessionParameters::new(),
    );
    assert!(
        turnovers.contains("(\"__aggregate_base\".\"_period\" < '2026-09-01') AND \"__aggregate_base\".\"_fld999\" = 0 GROUP BY"),
        "{turnovers}"
    );
}
