//! `ИЗ &Таблица`: a value table passed as a parameter, inlined as a CTE.
mod support;

use open_sdbl::metadata::{MetadataKind, MetadataSnapshot};
use open_sdbl::query::{
    ColumnKind, CompileOptions, MsSqlBackend, ParameterColumn, ParameterValue, PostgresBackend,
    QueryCompiler, QueryDiagnosticKind, QueryParameter, SessionParameters, TempTablesManager,
};
use support::*;

fn number(value: i128) -> ParameterValue {
    ParameterValue::Number {
        unscaled: value,
        scale: 0,
    }
}

fn text(value: &str) -> ParameterValue {
    ParameterValue::String(value.to_owned())
}

fn string_kind() -> ColumnKind {
    ColumnKind::String { length: None }
}

fn number_kind() -> ColumnKind {
    ColumnKind::Number {
        precision: None,
        scale: None,
    }
}

fn table(columns: &[(&str, ColumnKind)], rows: Vec<Vec<ParameterValue>>) -> ParameterValue {
    ParameterValue::Table {
        columns: columns
            .iter()
            .map(|(name, kind)| ParameterColumn::new(*name, kind.clone()))
            .collect(),
        rows,
    }
}

fn compile(
    snapshot: &MetadataSnapshot,
    source: &str,
    value: ParameterValue,
) -> Result<String, open_sdbl::query::QueryDiagnostic> {
    let parameters = [QueryParameter::new("Таблица", value)];
    let options = CompileOptions::new().parameters(&parameters);
    QueryCompiler::new(snapshot, PostgresBackend)
        .compile_with(source, &options)
        .map(|compiled| compiled.sql)
}

fn assert_contains(sql: &str, needle: &str) {
    assert!(sql.contains(needle), "expected {needle:?} in\n{sql}");
}

#[test]
fn inlines_the_rows_as_a_cte_of_the_statement() {
    let snapshot = reference_snapshot();
    let sql = compile(
        &snapshot,
        "ВЫБРАТЬ Т.Код КАК Код, Т.Количество КАК К, П.Code КАК Кд
         ИЗ &Таблица КАК Т
         ВНУТРЕННЕЕ СОЕДИНЕНИЕ Справочник.OpenSdblMetadataProbe КАК П ПО П.Code = Т.Код
         ГДЕ Т.Количество > 1;",
        table(
            &[("Код", string_kind()), ("Количество", number_kind())],
            vec![vec![text("A"), number(1)], vec![text("B"), number(2)]],
        ),
    )
    .unwrap();
    assert!(
        sql.starts_with(
            "WITH RECURSIVE \"__param_1\" AS (SELECT 1 AS \"__row\", CAST('A' AS text) AS \"Код\", CAST(1 AS numeric) AS \"Количество\" UNION ALL SELECT 2, 'B', 2) SELECT"
        ),
        "{sql}"
    );
    assert_contains(
        &sql,
        "FROM \"__param_1\" AS \"Т\" INNER JOIN \"_reference53\" AS \"П\" ON",
    );
    assert_contains(&sql, "WHERE (\"Т\".\"Количество\" > 1)");
}

#[test]
fn an_empty_table_answers_no_rows() {
    let snapshot = reference_snapshot();
    let sql = compile(
        &snapshot,
        "ВЫБРАТЬ Т.Код КАК Код ИЗ &Таблица КАК Т;",
        table(&[("Код", string_kind())], Vec::new()),
    )
    .unwrap();
    assert_contains(
        &sql,
        "\"__param_1\" AS (SELECT 1 AS \"__row\", CAST(NULL AS text) AS \"Код\" WHERE 1 = 0)",
    );
}

#[test]
fn references_of_several_objects_widen_to_the_payload() {
    let snapshot = reference_snapshot();
    let probe = snapshot
        .object_id(MetadataKind::Catalog, "OpenSdblMetadataProbe")
        .unwrap();
    let organisation = snapshot
        .object_id(MetadataKind::Catalog, "Организации")
        .unwrap();
    let one = ParameterValue::Reference {
        object: probe,
        id: [0x11; 16],
    };
    let two = ParameterValue::Reference {
        object: organisation,
        id: [0x22; 16],
    };
    let any = ColumnKind::Reference {
        targets: Vec::new(),
        runtime_typed: true,
    };
    let sql = compile(
        &snapshot,
        "ВЫБРАТЬ Т.Ссылка КАК С ИЗ &Таблица КАК Т;",
        table(
            &[("Ссылка", any)],
            vec![vec![one.clone()], vec![two.clone()]],
        ),
    )
    .unwrap();
    assert_contains(
        &sql,
        "|| decode('11111111111111111111111111111111', 'hex')) AS bytea) AS \"Ссылка\"",
    );
    // One target keeps the 16-byte identifier and refuses another object.
    let fixed = ColumnKind::Reference {
        targets: vec![probe],
        runtime_typed: false,
    };
    let single = compile(
        &snapshot,
        "ВЫБРАТЬ Т.Ссылка КАК С ИЗ &Таблица КАК Т;",
        table(&[("Ссылка", fixed.clone())], vec![vec![one]]),
    )
    .unwrap();
    assert_contains(
        &single,
        "CAST(decode('11111111111111111111111111111111', 'hex') AS bytea) AS \"Ссылка\"",
    );
    let foreign = compile(
        &snapshot,
        "ВЫБРАТЬ Т.Ссылка КАК С ИЗ &Таблица КАК Т;",
        table(&[("Ссылка", fixed)], vec![vec![two]]),
    )
    .unwrap_err();
    assert!(foreign.message().contains("of kind Reference"), "{foreign}");
}

#[test]
fn reports_unbound_and_malformed_tables() {
    let snapshot = reference_snapshot();
    let source = "ВЫБРАТЬ Т.Код КАК Код ИЗ &Таблица КАК Т;";
    for (value, message) in [
        (number(1), "must be bound to a value table"),
        (
            table(&[("Код", string_kind())], vec![vec![text("A"), number(1)]]),
            "has 2 values for 1 columns",
        ),
        (
            table(&[("Код", string_kind())], vec![vec![number(1)]]),
            "holds Number",
        ),
        (
            table(
                &[("Код", string_kind()), ("код", number_kind())],
                Vec::new(),
            ),
            "named twice",
        ),
        (table(&[], Vec::new()), "at least one column"),
        (
            table(&[("Код", ColumnKind::Null)], Vec::new()),
            "needs a scalar or reference kind",
        ),
    ] {
        let error = compile(&snapshot, source, value).unwrap_err();
        assert_eq!(error.kind(), QueryDiagnosticKind::Parameter, "{error}");
        assert!(error.message().contains(message), "{error}");
    }
    // A table where a scalar is expected.
    let scalar = compile(
        &snapshot,
        "ВЫБРАТЬ П.Code КАК К ИЗ Справочник.OpenSdblMetadataProbe КАК П ГДЕ П.Code = &Таблица;",
        table(&[("Код", string_kind())], Vec::new()),
    )
    .unwrap_err();
    assert_eq!(scalar.kind(), QueryDiagnosticKind::Parameter, "{scalar}");
}

#[test]
fn renders_for_sql_server_and_travels_with_a_temporary_table() {
    let snapshot = reference_snapshot();
    let parameters = [QueryParameter::new(
        "Таблица",
        table(
            &[("Код", string_kind())],
            vec![vec![text("A")], vec![text("B")]],
        ),
    )];
    let options = CompileOptions::new().parameters(&parameters);
    let mut manager = TempTablesManager::new();
    let batch = "ВЫБРАТЬ Т.Код КАК Код ПОМЕСТИТЬ ВТ ИЗ &Таблица КАК Т;
                 ВЫБРАТЬ ВТ.Код КАК Код ИЗ ВТ КАК ВТ;";
    let mssql = QueryCompiler::new(&snapshot, MsSqlBackend::new(2000).unwrap())
        .compile_batch(batch, &options, &mut manager)
        .unwrap()
        .unwrap()
        .sql;
    assert!(
        mssql.starts_with(
            "WITH [__param_1_t1] AS (SELECT 1 AS [__row], CAST(N'A' AS nvarchar(max)) AS [Код] UNION ALL SELECT 2, N'B'), [vt1] AS (SELECT [Т].[Код] AS [Код] FROM [__param_1_t1] AS [Т])"
        ),
        "{mssql}"
    );
}

#[test]
fn the_preparation_pass_sees_the_columns_the_text_names() {
    let snapshot = reference_snapshot();
    let prepared = QueryCompiler::new(&snapshot, PostgresBackend).prepare(
        "ВЫБРАТЬ Т.Код КАК Код, ПРЕДСТАВЛЕНИЕ(П.Ссылка) КАК Пр
         ИЗ &Таблица КАК Т
         ЛЕВОЕ СОЕДИНЕНИЕ Справочник.OpenSdblMetadataProbe КАК П ПО П.Code = Т.Код
         ГДЕ Т.Количество > 1;",
    );
    assert!(prepared.is_ok(), "{:?}", prepared.err());
}

/// The shape on a real base: the demo Бухгалтерия fixture, when present.
#[test]
fn joins_a_catalog_of_the_buh_fixture() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/buh");
    if !root.join("db_names.deflate").is_file() {
        return;
    }
    let snapshot = demo_resolved_at(&root).snapshot;
    let mut session = SessionParameters::new();
    for name in ["ОбластьДанныхЗначение", "ОбластьДанныхОсновныеДанные"]
    {
        session.set(QueryParameter::new(name, number(0)));
    }
    session.set(QueryParameter::new(
        "ОбластьДанныхИспользование",
        ParameterValue::Boolean(false),
    ));
    let parameters = [QueryParameter::new(
        "Таблица",
        table(
            &[("Код", string_kind()), ("Количество", number_kind())],
            vec![
                vec![text("00-00000001"), number(1)],
                vec![text("00-00000002"), number(2)],
            ],
        ),
    )];
    let options = CompileOptions::new()
        .session(&session)
        .parameters(&parameters);
    let sql = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile_with(
            "ВЫБРАТЬ Т.Код КАК Код, Н.Наименование КАК Имя, Т.Количество КАК К
             ИЗ &Таблица КАК Т
             ЛЕВОЕ СОЕДИНЕНИЕ Справочник.Номенклатура КАК Н ПО Н.Код = Т.Код;",
            &options,
        )
        .unwrap()
        .sql;
    println!("SQL {sql}");
    assert_contains(&sql, "LEFT JOIN \"_reference");
}
