//! Structured names of missing query and session parameters.

mod support;

use open_sdbl::query::{
    CompileOptions, ParameterValue, PostgresBackend, QueryCompiler, QueryDiagnosticKind,
    QueryParameter,
};

#[test]
fn an_unbound_query_parameter_keeps_its_name_and_position() {
    let snapshot = support::snapshot();
    let error = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile("SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE Code = &Код;")
        .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::Parameter);
    assert_eq!(error.missing_parameter_name(), Some("Код"));
    assert!(!error.is_missing_session_parameter());
    assert_eq!((error.line(), error.column()), (1, 61));
    assert!(error.message().contains("has no value"));
}

#[test]
fn an_unused_parameter_does_not_claim_to_be_missing() {
    let snapshot = support::snapshot();
    let parameters = [QueryParameter::new("Лишний", ParameterValue::Null)];
    let error = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile_with(
            "SELECT Code FROM Catalog.OpenSdblMetadataProbe;",
            &CompileOptions::new().parameters(&parameters),
        )
        .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::Parameter);
    assert_eq!(error.missing_parameter_name(), None);
    assert!(!error.is_missing_session_parameter());
}

#[test]
fn a_separator_failure_identifies_a_missing_session_parameter() {
    let snapshot = support::separators_snapshot();
    let error = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile("ВЫБРАТЬ Т.Ссылка ИЗ Справочник.Товары КАК Т")
        .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::Parameter);
    assert_eq!(error.missing_parameter_name(), Some("ЗначениеРазделителя"));
    assert!(error.is_missing_session_parameter());
}
