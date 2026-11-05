//! `Prepared::parameter_names`: the `&Имя` slots of a prepared text.
mod support;

use open_sdbl::metadata::ObjectId;
use open_sdbl::query::{
    AccessRestriction, Backend, CompileOptions, MsSqlBackend, ParameterValue, PostgresBackend,
    PrepareOptions, QueryCompiler, QueryParameter, SessionParameters, TempTablesManager,
    find_metadata_object,
};

fn names<B: Backend>(backend: B, source: &str) -> Vec<String> {
    let snapshot = support::accumulation_register_snapshot();
    QueryCompiler::new(&snapshot, backend)
        .prepare(source)
        .unwrap_or_else(|error| panic!("{source}: {error}"))
        .parameter_names()
        .to_vec()
}

fn both(source: &str) -> Vec<String> {
    let postgres = names(PostgresBackend, source);
    assert_eq!(postgres, names(MsSqlBackend::new(0).unwrap(), source));
    postgres
}

#[test]
fn first_occurrence_order_and_spelling_case_insensitively() {
    assert_eq!(both("ВЫБРАТЬ 1 ГДЕ &Б > 0 И &а > 0 И &б < 5"), ["Б", "а"]);
    assert_eq!(
        both("SELECT &Second, &first, &SECOND, &First"),
        ["Second", "first"]
    );
}

#[test]
fn literals_and_comments_hold_no_parameters() {
    assert!(both("ВЫБРАТЬ 1 ГДЕ \"&Имя\" = \"x\"").is_empty());
    assert!(both("ВЫБРАТЬ 1 // &Имя\nГДЕ ИСТИНА").is_empty());
    assert_eq!(both("ВЫБРАТЬ &Видимый // &Скрытый"), ["Видимый"]);
}

#[test]
fn batches_list_every_statement_in_order() {
    let source = "ВЫБРАТЬ &Дата КАК Д ПОМЕСТИТЬ ВТ; ВЫБРАТЬ Д, &Склад ИЗ ВТ";
    assert_eq!(both(source), ["Дата", "Склад"]);
}

#[test]
fn restriction_session_parameters_are_not_listed() {
    let snapshot = support::accumulation_register_snapshot();
    let object = find_metadata_object(&snapshot, "AccumulationRegister.Остатки").unwrap();
    let restrictions = [AccessRestriction::new(
        ObjectId::from(&object.guid),
        "ГДЕ ИСТИНА = &ДоступнаОбласть",
    )];
    let source =
        "SELECT ALLOWED Номенклатура FROM AccumulationRegister.Остатки.Balance() WHERE &Отбор";
    let manager = TempTablesManager::new();
    let prepared = QueryCompiler::new(&snapshot, PostgresBackend)
        .prepare_with_options(source, &PrepareOptions::new().temporary_tables(&manager))
        .unwrap();
    assert_eq!(prepared.parameter_names(), ["Отбор"]);
    let mut session = SessionParameters::new();
    session.set(QueryParameter::new(
        "ДоступнаОбласть",
        ParameterValue::Boolean(true),
    ));
    let values = [QueryParameter::new("Отбор", ParameterValue::Boolean(true))];
    prepared
        .compile_with(
            &snapshot,
            &CompileOptions::new()
                .restrictions(&restrictions)
                .session(&session)
                .parameters(&values),
        )
        .unwrap();
    // Compiling with the restriction changes nothing in the list.
    assert_eq!(prepared.parameter_names(), ["Отбор"]);
}

#[test]
fn a_dollar_placeholder_is_a_lexing_error() {
    let snapshot = support::accumulation_register_snapshot();
    let error = QueryCompiler::new(&snapshot, PostgresBackend)
        .prepare("ВЫБРАТЬ 1 ГДЕ $1 > 0")
        .unwrap_err();
    assert_eq!(error.kind(), open_sdbl::query::QueryDiagnosticKind::Lex);
}
