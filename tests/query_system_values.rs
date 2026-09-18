//! `ЗНАЧЕНИЕ` over the system enumerations and the standard fields of a
//! chart of accounts.

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

/// Compiles against the UNF fixture, whose data separators are switched
/// off through the session parameters.
fn postgres_unf(snapshot: &MetadataSnapshot, source: &str) -> String {
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
    let options = CompileOptions::new().session(&session);
    QueryCompiler::new(snapshot, PostgresBackend)
        .compile_with(source, &options)
        .unwrap_or_else(|error| panic!("{source}: {error}"))
        .sql
}

fn assert_contains(sql: &str, needle: &str) {
    assert!(sql.contains(needle), "expected {needle:?} in\n{sql}");
}

#[test]
fn compares_the_record_kind_with_a_system_value() {
    let snapshot = accumulation_register_snapshot();
    let sql = postgres(
        &snapshot,
        "ВЫБРАТЬ Т.Количество КАК К ИЗ РегистрНакопления.Остатки КАК Т
         ГДЕ Т.ВидДвижения = ЗНАЧЕНИЕ(ВидДвиженияНакопления.Расход);",
    );
    assert_contains(&sql, "\"_recordkind\" = 1");
    let english = postgres(
        &snapshot,
        "SELECT T.Количество AS K FROM AccumulationRegister.Остатки AS T
         WHERE T.RecordType = VALUE(AccumulationRecordType.Receipt);",
    );
    assert_contains(&english, "\"_recordkind\" = 0");
    let mssql = QueryCompiler::new(&snapshot, MsSqlBackend::new(2000).unwrap())
        .compile(
            "ВЫБРАТЬ Т.Количество КАК К ИЗ РегистрНакопления.Остатки КАК Т
             ГДЕ Т.ВидДвижения = ЗНАЧЕНИЕ(ВидДвиженияНакопления.Расход);",
        )
        .unwrap()
        .sql;
    assert_contains(&mssql, "[_recordkind] = 1");
}

#[test]
fn system_values_are_numbers_in_a_projection() {
    let snapshot = accumulation_register_snapshot();
    let sql = postgres(
        &snapshot,
        "ВЫБРАТЬ ЗНАЧЕНИЕ(ВидСчета.АктивноПассивный) КАК Вид,
                 ЗНАЧЕНИЕ(ВидДвиженияБухгалтерии.Кредит) КАК Сторона;",
    );
    assert_contains(&sql, "2 AS \"Вид\"");
    assert_contains(&sql, "1 AS \"Сторона\"");
}

#[test]
fn an_unknown_system_value_is_a_syntax_error() {
    let snapshot = accumulation_register_snapshot();
    let error = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile("ВЫБРАТЬ ЗНАЧЕНИЕ(ВидСчета.Дебет) КАК Вид;")
        .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::Syntax);
    assert!(error.message().contains("Дебет"), "{}", error.message());
}

/// The UNF fixture carries a chart of accounts; a chart of the unit
/// fixtures does not.
fn unf() -> Option<MetadataSnapshot> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/unf");
    root.join("db_names.deflate")
        .is_file()
        .then(|| demo_resolved_at(&root).snapshot)
}

#[test]
fn exposes_the_standard_fields_of_a_chart_of_accounts() {
    let Some(snapshot) = unf() else {
        return;
    };
    let sql = postgres_unf(
        &snapshot,
        "ВЫБРАТЬ С.Код КАК Код, С.Вид КАК Вид, С.Порядок КАК Порядок
         ИЗ ПланСчетов.Управленческий КАК С
         ГДЕ НЕ С.Забалансовый И С.Вид = ЗНАЧЕНИЕ(ВидСчета.Пассивный);",
    );
    assert_contains(&sql, "\"_kind\" AS \"Вид\"");
    assert_contains(&sql, "\"_orderfield\"::text AS \"Порядок\"");
    assert_contains(&sql, "\"_offbalance\"");
    assert_contains(&sql, "\"_kind\" = 1");
    let dereferenced = postgres_unf(
        &snapshot,
        "ВЫБРАТЬ С.Родитель.Вид КАК Вид ИЗ ПланСчетов.Управленческий КАК С;",
    );
    assert_contains(&dereferenced, "\"_kind\" AS \"Вид\"");
}
