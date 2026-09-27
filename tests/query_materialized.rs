mod support;

use open_sdbl::query::{
    Backend, CompileOptions, MaterializedBatchResult, MsSqlBackend, ParameterValue,
    PostgresBackend, QueryCompiler, QueryDiagnostic, QueryDiagnosticKind, TempTablesManager,
};
use support::snapshot;

fn snapshot_and_append<B: Backend>(backend: B, quote: &str) {
    let snapshot = snapshot();
    let compiler = QueryCompiler::new(&snapshot, backend);
    let mut manager = TempTablesManager::new();
    let mut calls = 0;
    let result = compiler
        .compile_materialized_batch(
            "ВЫБРАТЬ Code КАК Код ПОМЕСТИТЬ ВТ ИЗ Catalog.OpenSdblMetadataProbe;",
            &CompileOptions::new(),
            &mut manager,
            |selection| {
                calls += 1;
                assert!(selection.sql.contains("_reference53"));
                Ok::<_, QueryDiagnostic>(vec![vec![ParameterValue::String("Before".into())]])
            },
        )
        .unwrap();
    assert_eq!(result, MaterializedBatchResult::Placed(1));
    assert_eq!(calls, 1);
    let result = compiler
        .compile_materialized_batch(
            "ВЫБРАТЬ Т.Код ИЗ ВТ КАК Т;",
            &CompileOptions::new(),
            &mut manager,
            |_| -> Result<_, QueryDiagnostic> { panic!("read must not place again") },
        )
        .unwrap();
    let MaterializedBatchResult::Query(query) = result else {
        panic!("expected query")
    };
    assert!(query.sql.contains("Before"));
    assert!(!query.sql.contains("_reference53"));
    assert!(query.sql.contains(quote));

    let result = compiler
        .compile_materialized_batch(
            "ВЫБРАТЬ Code КАК Код ДОБАВИТЬ ВТ ИЗ Catalog.OpenSdblMetadataProbe;",
            &CompileOptions::new(),
            &mut manager,
            |_| Ok::<_, QueryDiagnostic>(vec![vec![ParameterValue::String("After".into())]]),
        )
        .unwrap();
    assert_eq!(result, MaterializedBatchResult::Placed(1));
    let result = compiler
        .compile_materialized_batch(
            "ВЫБРАТЬ Т.Код ИЗ ВТ КАК Т;",
            &CompileOptions::new(),
            &mut manager,
            |_| -> Result<_, QueryDiagnostic> { panic!("read must not place again") },
        )
        .unwrap();
    let MaterializedBatchResult::Query(query) = result else {
        panic!("expected query")
    };
    assert!(query.sql.contains("Before") && query.sql.contains("After"));
    assert!(!query.sql.contains("_reference53"));
}

#[test]
fn postgres_captures_rows_once_and_appends_only_new_rows() {
    snapshot_and_append(PostgresBackend, "\"vt1\"");
}

#[test]
fn mssql_captures_rows_once_and_appends_only_new_rows() {
    snapshot_and_append(MsSqlBackend::new(2000).unwrap(), "[vt1]");
}

fn packet_and_drop<B: Backend>(backend: B) {
    let snapshot = snapshot();
    let compiler = QueryCompiler::new(&snapshot, backend);
    let mut manager = TempTablesManager::new();
    let mut calls = 0;
    let result = compiler
        .compile_materialized_batch(
            "ВЫБРАТЬ Code КАК Код ПОМЕСТИТЬ Первая ИЗ Catalog.OpenSdblMetadataProbe;
         ВЫБРАТЬ Т.Код КАК Код ПОМЕСТИТЬ Вторая ИЗ Первая КАК Т;
         ВЫБРАТЬ Т.Код ИЗ Вторая КАК Т;",
            &CompileOptions::new(),
            &mut manager,
            |selection| {
                calls += 1;
                if calls == 2 {
                    assert!(selection.sql.contains("Before"));
                    assert!(!selection.sql.contains("_reference53"));
                }
                Ok::<_, QueryDiagnostic>(vec![vec![ParameterValue::String("Before".into())]])
            },
        )
        .unwrap();
    assert_eq!(calls, 2);
    let MaterializedBatchResult::Query(query) = result else {
        panic!("expected query")
    };
    assert!(query.sql.contains("Before"));
    assert!(!query.sql.contains("_reference53"));
    assert_eq!(manager.tables().count(), 2);
    assert_eq!(
        compiler
            .compile_materialized_batch(
                "УНИЧТОЖИТЬ Первая;",
                &CompileOptions::new(),
                &mut manager,
                |_| -> Result<_, QueryDiagnostic> { panic!("drop cannot place") },
            )
            .unwrap(),
        MaterializedBatchResult::Dropped
    );
    assert!(!manager.contains("Первая"));
    assert!(manager.contains("Вторая"));
    let before = manager.clone();
    let error = compiler
        .compile_materialized_batch(
            "УНИЧТОЖИТЬ Вторая; ВЫБРАТЬ Т.Код ИЗ Вторая КАК Т;",
            &CompileOptions::new(),
            &mut manager,
            |_| -> Result<_, QueryDiagnostic> { panic!("bad batch cannot place") },
        )
        .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::TemporaryTable);
    assert_eq!(manager, before);
}

#[test]
fn postgres_packet_dependency_and_drop() {
    packet_and_drop(PostgresBackend);
}

#[test]
fn mssql_packet_dependency_and_drop() {
    packet_and_drop(MsSqlBackend::new(2000).unwrap());
}

fn wrong_rows_roll_back<B: Backend>(backend: B) {
    let snapshot = snapshot();
    let compiler = QueryCompiler::new(&snapshot, backend);
    let mut manager = TempTablesManager::new();
    let error = compiler
        .compile_materialized_batch(
            "ВЫБРАТЬ Code КАК Код ПОМЕСТИТЬ ВТ ИЗ Catalog.OpenSdblMetadataProbe;",
            &CompileOptions::new(),
            &mut manager,
            |_| Ok::<_, QueryDiagnostic>(vec![vec![ParameterValue::Boolean(true)]]),
        )
        .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::TemporaryTable);
    assert!(manager.is_empty());
    let result = compiler
        .compile_materialized_batch(
            "ВЫБРАТЬ Code КАК Код ПОМЕСТИТЬ ВТ ИЗ Catalog.OpenSdblMetadataProbe;",
            &CompileOptions::new(),
            &mut manager,
            |_| Ok::<_, QueryDiagnostic>(vec![]),
        )
        .unwrap();
    assert_eq!(result, MaterializedBatchResult::Placed(0));
    let MaterializedBatchResult::Query(query) = compiler
        .compile_materialized_batch(
            "ВЫБРАТЬ Т.Код ИЗ ВТ КАК Т;",
            &CompileOptions::new(),
            &mut manager,
            |_| -> Result<_, QueryDiagnostic> { panic!("read cannot place") },
        )
        .unwrap()
    else {
        panic!("expected query")
    };
    assert!(query.sql.contains("WHERE 1 = 0"));
}

#[test]
fn postgres_rejects_wrong_rows_and_preserves_empty_types() {
    wrong_rows_roll_back(PostgresBackend);
}

#[test]
fn mssql_rejects_wrong_rows_and_preserves_empty_types() {
    wrong_rows_roll_back(MsSqlBackend::new(2000).unwrap());
}

fn null_and_budget<B: Backend>(backend: B) {
    let snapshot = snapshot();
    let compiler = QueryCompiler::new(&snapshot, backend);
    let mut manager = TempTablesManager::new();
    let result = compiler
        .compile_materialized_batch(
            "ВЫБРАТЬ Code КАК Код ПОМЕСТИТЬ ВТ ИЗ Catalog.OpenSdblMetadataProbe;",
            &CompileOptions::new(),
            &mut manager,
            |_| Ok::<_, QueryDiagnostic>(vec![vec![ParameterValue::Null]]),
        )
        .unwrap();
    assert_eq!(result, MaterializedBatchResult::Placed(1));
    let MaterializedBatchResult::Query(query) = compiler
        .compile_materialized_batch(
            "ВЫБРАТЬ Т.Код ИЗ ВТ КАК Т;",
            &CompileOptions::new(),
            &mut manager,
            |_| -> Result<_, QueryDiagnostic> { panic!("read cannot place") },
        )
        .unwrap()
    else {
        panic!("expected query")
    };
    assert!(query.sql.contains("CAST(NULL AS"));
    let before = manager.clone();
    let error = compiler
        .compile_materialized_batch(
            "ВЫБРАТЬ Code КАК Код ПОМЕСТИТЬ Большая ИЗ Catalog.OpenSdblMetadataProbe;",
            &CompileOptions::new(),
            &mut manager,
            |_| {
                Ok::<_, QueryDiagnostic>(vec![vec![ParameterValue::String(
                    "x".repeat(8 * 1024 * 1024),
                )]])
            },
        )
        .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::TemporaryTable);
    assert_eq!(manager, before);
}

#[test]
fn postgres_null_and_memory_budget() {
    null_and_budget(PostgresBackend);
}

#[test]
fn mssql_null_and_memory_budget() {
    null_and_budget(MsSqlBackend::new(2000).unwrap());
}

fn cte_only_entries_are_rejected<B: Backend>(backend: B) {
    let snapshot = snapshot();
    let compiler = QueryCompiler::new(&snapshot, backend);
    let mut manager = TempTablesManager::new();
    compiler
        .compile_batch(
            "ВЫБРАТЬ Code КАК Код ПОМЕСТИТЬ ВТ ИЗ Catalog.OpenSdblMetadataProbe;",
            &CompileOptions::new(),
            &mut manager,
        )
        .unwrap();
    let before = manager.clone();
    let error = compiler
        .compile_materialized_batch(
            "ВЫБРАТЬ Т.Код ИЗ ВТ КАК Т;",
            &CompileOptions::new(),
            &mut manager,
            |_| -> Result<_, QueryDiagnostic> { panic!("read cannot place") },
        )
        .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::TemporaryTable);
    assert_eq!(manager, before);
}

#[test]
fn postgres_does_not_reread_legacy_table() {
    cte_only_entries_are_rejected(PostgresBackend);
}

#[test]
fn mssql_does_not_reread_legacy_table() {
    cte_only_entries_are_rejected(MsSqlBackend::new(2000).unwrap());
}

fn later_missing_session_parameter_prevents_any_callback<B: Backend>(backend: B) {
    let snapshot = support::separators_snapshot();
    let compiler = QueryCompiler::new(&snapshot, backend);
    let mut manager = TempTablesManager::new();
    let mut calls = 0;
    let error = compiler
        .compile_materialized_batch(
            "ВЫБРАТЬ 1 КАК Число ПОМЕСТИТЬ ВТ;
         ВЫБРАТЬ Т.Ссылка ИЗ Справочник.Товары КАК Т;",
            &CompileOptions::new(),
            &mut manager,
            |_| {
                calls += 1;
                Ok::<_, QueryDiagnostic>(vec![vec![ParameterValue::Number {
                    unscaled: 1,
                    scale: 0,
                }]])
            },
        )
        .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::Parameter);
    assert!(error.is_missing_session_parameter());
    assert_eq!(error.missing_parameter_name(), Some("ЗначениеРазделителя"));
    assert_eq!(calls, 0);
    assert!(manager.is_empty());
}

#[test]
fn postgres_preflights_later_session_parameter() {
    later_missing_session_parameter_prevents_any_callback(PostgresBackend);
}

#[test]
fn mssql_preflights_later_session_parameter() {
    later_missing_session_parameter_prevents_any_callback(MsSqlBackend::new(2000).unwrap());
}

#[test]
fn mssql_callback_sees_logical_date_and_retention_restores_storage_date() {
    use open_sdbl::query::ParameterDate;
    let snapshot = snapshot();
    let compiler = QueryCompiler::new(&snapshot, MsSqlBackend::new(2000).unwrap());
    let mut manager = TempTablesManager::new();
    let result = compiler
        .compile_materialized_batch(
            "ВЫБРАТЬ Date КАК Дата ПОМЕСТИТЬ ВТ ИЗ Catalog.OpenSdblMetadataProbe;",
            &CompileOptions::new(),
            &mut manager,
            |selection| {
                assert!(selection.sql.contains("DATEADD(year, -2000"));
                Ok::<_, QueryDiagnostic>(vec![vec![ParameterValue::Date(
                    ParameterDate::new(2024, 2, 3, 4, 5, 6).unwrap(),
                )]])
            },
        )
        .unwrap();
    assert_eq!(result, MaterializedBatchResult::Placed(1));
    let MaterializedBatchResult::Query(query) = compiler
        .compile_materialized_batch(
            "ВЫБРАТЬ Т.Дата ИЗ ВТ КАК Т;",
            &CompileOptions::new(),
            &mut manager,
            |_| -> Result<_, QueryDiagnostic> { panic!("read cannot place") },
        )
        .unwrap()
    else {
        panic!("expected query")
    };
    assert!(query.sql.contains("DATEADD(year, 2000"));
    assert!(!query.sql.contains("_reference53"));
}

fn retains_fixed_reference<B: Backend>(backend: B, literal: &str) {
    use open_sdbl::metadata::MetadataKind;
    let snapshot = snapshot();
    let object = snapshot
        .object_id(MetadataKind::Catalog, "OpenSdblMetadataProbe")
        .unwrap();
    let compiler = QueryCompiler::new(&snapshot, backend);
    let mut manager = TempTablesManager::new();
    assert_eq!(
        compiler
            .compile_materialized_batch(
                "ВЫБРАТЬ Ссылка КАК Объект ПОМЕСТИТЬ ВТ ИЗ Catalog.OpenSdblMetadataProbe;",
                &CompileOptions::new(),
                &mut manager,
                |_| Ok::<_, QueryDiagnostic>(vec![vec![ParameterValue::Reference {
                    object,
                    id: [1; 16]
                }]]),
            )
            .unwrap(),
        MaterializedBatchResult::Placed(1)
    );
    let MaterializedBatchResult::Query(query) = compiler
        .compile_materialized_batch(
            "ВЫБРАТЬ Т.Объект ИЗ ВТ КАК Т;",
            &CompileOptions::new(),
            &mut manager,
            |_| -> Result<_, QueryDiagnostic> { panic!("read cannot place") },
        )
        .unwrap()
    else {
        panic!("expected query")
    };
    assert!(query.sql.contains(literal), "{}", query.sql);
    assert!(!query.sql.contains("_reference53"));
}

#[test]
fn postgres_retains_fixed_reference() {
    retains_fixed_reference(PostgresBackend, "decode('01010101");
}

#[test]
fn mssql_retains_fixed_reference() {
    retains_fixed_reference(MsSqlBackend::new(2000).unwrap(), "0x01010101");
}

#[derive(Debug)]
enum HostFailure {
    Query,
    Database,
}

impl From<QueryDiagnostic> for HostFailure {
    fn from(_: QueryDiagnostic) -> Self {
        Self::Query
    }
}

fn callback_failure_keeps_manager<B: Backend>(backend: B) {
    let snapshot = snapshot();
    let compiler = QueryCompiler::new(&snapshot, backend);
    let mut manager = TempTablesManager::new();
    let error = compiler
        .compile_materialized_batch(
            "ВЫБРАТЬ Code КАК Код ПОМЕСТИТЬ ВТ ИЗ Catalog.OpenSdblMetadataProbe;",
            &CompileOptions::new(),
            &mut manager,
            |_| -> Result<Vec<Vec<ParameterValue>>, HostFailure> { Err(HostFailure::Database) },
        )
        .unwrap_err();
    assert!(matches!(error, HostFailure::Database));
    assert!(manager.is_empty());
}

#[test]
fn postgres_callback_failure_keeps_manager() {
    callback_failure_keeps_manager(PostgresBackend);
}

#[test]
fn mssql_callback_failure_keeps_manager() {
    callback_failure_keeps_manager(MsSqlBackend::new(2000).unwrap());
}
