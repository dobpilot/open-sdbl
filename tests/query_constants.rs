//! The `Константы` source: one row holding the constants a statement reads.
//!
//! The snapshot comes from the probe base under `tests/fixtures/separators`
//! (constant `ОсновнойТовар`, a reference to catalog `Товары`, and two
//! separators); `with_second_constant` adds a numeric `ВтораяКонстанта`.

mod support;

use support::*;

use open_sdbl::metadata::{
    ConfigDescriptor, LiveColumn, LiveTable, MetadataSnapshot, parse_db_names, resolve_metadata,
};
use open_sdbl::query::{
    Backend, ColumnKind, CompileOptions, CompiledQuery, MsSqlBackend, ParameterValue,
    PostgresBackend, QueryCompiler, QueryDiagnostic, QueryDiagnosticKind, QueryParameter,
    SessionParameters, constants_table_fields,
};

const SECOND: &str = "a1b2c3d4-0070-4000-8000-000000000070";

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

fn area(value: i128) -> SessionParameters {
    let mut session = SessionParameters::new();
    session.set(QueryParameter::new(
        "ЗначениеРазделителя",
        ParameterValue::Number {
            unscaled: value,
            scale: 0,
        },
    ));
    session
}

fn postgres_sql(snapshot: &MetadataSnapshot, source: &str) -> String {
    compile(
        snapshot,
        PostgresBackend,
        source,
        &CompileOptions::new().session(&area(7)),
    )
    .unwrap_or_else(|error| panic!("{source}: {error}"))
    .sql
}

/// Adds the numeric constant `ВтораяКонстанта` (`_Const70`, value
/// `_Fld71`) with both separator columns to the probe snapshot.
fn with_second_constant(base: MetadataSnapshot) -> MetadataSnapshot {
    let entries = base
        .db_names()
        .entries()
        .iter()
        .map(|entry| format!("{{{},\"{}\",{}}}", entry.guid, entry.alias, entry.number))
        .chain([
            format!("{{{SECOND},\"Const\",70}}"),
            format!("{{{SECOND},\"Fld\",71}}"),
        ])
        .collect::<Vec<_>>();
    let serialized = format!("{{{},{}}}", entries.len(), entries.join(","));
    let db_names = parse_db_names(&stored_deflate(serialized.as_bytes())).unwrap();
    let mut descriptors = base.descriptors().to_vec();
    descriptors.push(ConfigDescriptor {
        resource_guid: guid(SECOND),
        object_guid: guid(SECOND),
        marker: "1".to_owned(),
        name: "ВтораяКонстанта".to_owned(),
        synonyms: Vec::new(),
        comment: None,
        field_purpose: None,
        enumeration_value: false,
        separation: None,
        reference_types: Vec::new(),
        object_reference_type: None,
        balance: None,
        chart_of_accounts: None,
    });
    let mut schema = base.schema().clone();
    schema.tables.push(schema_table(
        "Const70",
        70,
        vec![
            schema_column("Fld71", "N", None),
            schema_column("Fld56", "N", None),
            schema_column("Fld57", "N", None),
            schema_column("RecordKey", "B", None),
        ],
    ));
    let mut live_tables = base.live_tables().to_vec();
    live_tables.push(LiveTable {
        name: "_const70".to_owned(),
        columns: [
            ("_fld71", "numeric(10,2)"),
            ("_fld56", "numeric(7,0)"),
            ("_fld57", "numeric(7,0)"),
            ("_recordkey", "bytea"),
        ]
        .iter()
        .map(|(name, data_type)| LiveColumn {
            name: (*name).to_owned(),
            data_type: (*data_type).to_owned(),
        })
        .collect(),
        indexes: Vec::new(),
    });
    resolve_metadata(db_names, descriptors, schema, live_tables).snapshot
}

#[test]
fn lists_every_live_constant_with_its_value_field() {
    let snapshot = with_second_constant(separators_snapshot());
    let constants = constants_table_fields(&snapshot).unwrap();
    let names = constants
        .iter()
        .map(|(object, field)| (object.name.as_deref().unwrap(), field.name.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            ("ВтораяКонстанта", "ВтораяКонстанта"),
            ("ОсновнойТовар", "ОсновнойТовар")
        ]
    );
    assert_eq!(constants[1].1.schema_name, "Fld62");
    assert_eq!(
        constants[1].1.reference_target.as_deref(),
        Some("Reference53")
    );
}

#[test]
fn reads_only_the_referenced_constants_as_one_aggregated_row() {
    let snapshot = with_second_constant(separators_snapshot());
    let sql = postgres_sql(
        &snapshot,
        "ВЫБРАТЬ К.ВтораяКонстанта, К.ОсновнойТовар ИЗ Константы КАК К",
    );
    assert_eq!(
        sql,
        "SELECT \"К\".\"_fld71\" AS \"ВтораяКонстанта\", \"К\".\"_fld62rref\" AS \"ОсновнойТовар\" FROM (SELECT MAX(\"__constants\".\"_fld71\") AS \"_fld71\", MAX(\"__constants\".\"_fld62rref\") AS \"_fld62rref\" FROM (SELECT \"__constant\".\"_fld71\" AS \"_fld71\", CAST(NULL AS bytea) AS \"_fld62rref\" FROM \"_const70\" AS \"__constant\" WHERE \"__constant\".\"_fld56\" = 7 AND \"__constant\".\"_fld57\" = 7 UNION ALL SELECT CAST(NULL AS numeric(10,2)) AS \"_fld71\", \"__constant\".\"_fld62rref\" AS \"_fld62rref\" FROM \"_const61\" AS \"__constant\" WHERE \"__constant\".\"_fld56\" = 7 AND \"__constant\".\"_fld57\" = 7) AS \"__constants\") AS \"К\""
    );
    let single = postgres_sql(&snapshot, "ВЫБРАТЬ К.ОсновнойТовар ИЗ Константы КАК К");
    assert!(!single.contains("_const70"), "{single}");
    assert!(
        single.contains("FROM \"_const61\" AS \"__constant\""),
        "{single}"
    );

    let mssql = compile(
        &snapshot,
        mssql(),
        "ВЫБРАТЬ К.ВтораяКонстанта ИЗ Константы КАК К",
        &CompileOptions::new().session(&area(7)),
    )
    .unwrap();
    assert_eq!(
        mssql.sql,
        "SELECT [К].[_fld71] AS [ВтораяКонстанта] FROM (SELECT MAX([__constants].[_fld71]) AS [_fld71] FROM (SELECT [__constant].[_fld71] AS [_fld71] FROM [_const70] AS [__constant] WHERE [__constant].[_fld56] = 7 AND [__constant].[_fld57] = 7) AS [__constants]) AS [К]"
    );
    assert_eq!(
        mssql.columns[0].kind,
        ColumnKind::Number {
            precision: Some(10),
            scale: Some(2)
        }
    );
}

#[test]
fn wildcard_reads_every_constant_and_an_unqualified_source_is_named_constants() {
    let snapshot = with_second_constant(separators_snapshot());
    let star = postgres_sql(&snapshot, "ВЫБРАТЬ * ИЗ Константы");
    assert!(star.starts_with("SELECT \"__src\".\"_fld71\" AS \"ВтораяКонстанта\", \"__src\".\"_fld62rref\" AS \"ОсновнойТовар\" FROM (SELECT MAX("), "{star}");
    assert!(
        star.contains("FROM \"_const70\" AS \"__constant\""),
        "{star}"
    );
    assert!(
        star.contains("FROM \"_const61\" AS \"__constant\""),
        "{star}"
    );

    let qualified = postgres_sql(
        &snapshot,
        "ВЫБРАТЬ Constants.ОсновнойТовар.Артикул КАК А ИЗ Constants",
    );
    assert_eq!(
        qualified,
        "SELECT \"__ref1\".\"_fld54\"::text AS \"А\" FROM (SELECT MAX(\"__constants\".\"_fld62rref\") AS \"_fld62rref\" FROM (SELECT \"__constant\".\"_fld62rref\" AS \"_fld62rref\" FROM \"_const61\" AS \"__constant\" WHERE \"__constant\".\"_fld56\" = 7 AND \"__constant\".\"_fld57\" = 7) AS \"__constants\") AS \"__src\" LEFT JOIN \"_reference53\" AS \"__ref1\" ON \"__src\".\"_fld62rref\" = \"__ref1\".\"_idrref\" AND \"__ref1\".\"_fld56\" = 7 AND \"__ref1\".\"_fld57\" = 7"
    );
}

#[test]
fn a_statement_without_constant_fields_still_sees_one_row() {
    let snapshot = separators_snapshot();
    assert_eq!(
        postgres_sql(&snapshot, "ВЫБРАТЬ КОЛИЧЕСТВО(*) КАК Н ИЗ Константы"),
        "SELECT COUNT(*) AS \"Н\" FROM (SELECT 1 AS \"__constants_row\") AS \"__src\""
    );
}

#[test]
fn joins_the_constants_table_with_a_catalog() {
    let snapshot = separators_snapshot();
    assert_eq!(
        postgres_sql(
            &snapshot,
            "ВЫБРАТЬ Т.Артикул ИЗ Справочник.Товары КАК Т ВНУТРЕННЕЕ СОЕДИНЕНИЕ Константы КАК К ПО Т.Ссылка = К.ОсновнойТовар"
        ),
        "SELECT \"Т\".\"_fld54\"::text AS \"Артикул\" FROM \"_reference53\" AS \"Т\" INNER JOIN (SELECT MAX(\"__constants\".\"_fld62rref\") AS \"_fld62rref\" FROM (SELECT \"__constant\".\"_fld62rref\" AS \"_fld62rref\" FROM \"_const61\" AS \"__constant\" WHERE \"__constant\".\"_fld56\" = 7 AND \"__constant\".\"_fld57\" = 7) AS \"__constants\") AS \"К\" ON \"Т\".\"_idrref\" = \"К\".\"_fld62rref\" WHERE \"Т\".\"_fld56\" = 7 AND \"Т\".\"_fld57\" = 7"
    );
}

#[test]
fn diagnoses_unknown_constants_and_disabled_separators() {
    let snapshot = separators_snapshot();
    let unknown = compile(
        &snapshot,
        PostgresBackend,
        "ВЫБРАТЬ К.Нет ИЗ Константы КАК К",
        &CompileOptions::new().session(&area(7)),
    )
    .unwrap_err();
    assert_eq!(unknown.kind(), QueryDiagnosticKind::UnknownField);
    assert_eq!((unknown.line(), unknown.column()), (1, 11));

    let mut disabled = SessionParameters::new();
    disabled.set(QueryParameter::new(
        "ИспользованиеРазделителя",
        ParameterValue::Boolean(false),
    ));
    let error = compile(
        &snapshot,
        PostgresBackend,
        "ВЫБРАТЬ К.ОсновнойТовар ИЗ Константы КАК К",
        &CompileOptions::new().session(&disabled),
    )
    .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::UnsupportedFeature);
    assert_eq!((error.line(), error.column()), (1, 28));
    assert_eq!(
        error.to_string(),
        "1:28: constant \"ОсновнойТовар\" is separated; the constants table needs a separator value"
    );
    let count = compile(
        &snapshot,
        PostgresBackend,
        "ВЫБРАТЬ КОЛИЧЕСТВО(*) КАК Н ИЗ Константы",
        &CompileOptions::new().session(&disabled),
    );
    assert!(count.is_ok(), "{count:?}");
}

/// `Константа.<Имя>` answers to `Значение`, not to the name of the
/// constant: measured on 8.3.27, `ВЫБРАТЬ * ИЗ Константа.ОсновнойТовар`
/// yields the column `Значение`, while `К.ОсновнойТовар` fails with
/// "Поле не найдено". The `Константы` table is the other way round.
#[test]
fn a_single_constant_source_answers_to_значение() {
    let snapshot = separators_snapshot();
    assert_eq!(
        postgres_sql(
            &snapshot,
            "ВЫБРАТЬ К.Значение ИЗ Константа.ОсновнойТовар КАК К"
        ),
        "SELECT \"К\".\"_fld62rref\" AS \"Значение\" FROM \"_const61\" AS \"К\" WHERE \"К\".\"_fld56\" = 7 AND \"К\".\"_fld57\" = 7"
    );
    assert_eq!(
        postgres_sql(&snapshot, "SELECT К.Value FROM Constant.ОсновнойТовар AS К"),
        "SELECT \"К\".\"_fld62rref\" AS \"Value\" FROM \"_const61\" AS \"К\" WHERE \"К\".\"_fld56\" = 7 AND \"К\".\"_fld57\" = 7"
    );

    let by_constant_name = compile(
        &snapshot,
        PostgresBackend,
        "ВЫБРАТЬ К.ОсновнойТовар ИЗ Константа.ОсновнойТовар КАК К",
        &CompileOptions::new().session(&area(7)),
    )
    .unwrap_err();
    assert_eq!(by_constant_name.kind(), QueryDiagnosticKind::UnknownField);
}
