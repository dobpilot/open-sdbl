//! Измеренное запрещение независимого разделителя до исполнения SQL.
mod support;
use open_sdbl::query::{
    Backend, CompileOptions, MsSqlBackend, ParameterValue, PostgresBackend, QueryCompiler,
    QueryDiagnosticKind, QueryParameter, SessionParameters,
};

fn disabled<B: Backend>(backend: B) {
    let snapshot = support::separators_snapshot();
    let mut session = SessionParameters::new();
    session.set(QueryParameter::new(
        "ИспользованиеРазделителя",
        ParameterValue::Boolean(false),
    ));
    let compiler = QueryCompiler::new(&snapshot, backend);
    let error = compiler
        .compile_with(
            "ВЫБРАТЬ Ссылка ИЗ Справочник.Товары",
            &CompileOptions::new().session(&session),
        )
        .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::Metadata);
    assert!(!error.is_missing_session_parameter());
    assert_eq!(error.missing_parameter_name(), None);
    assert!(error.message().contains("independent usage"));
    session.set(QueryParameter::new(
        "ЗначениеРазделителя",
        ParameterValue::Number {
            unscaled: 7,
            scale: 0,
        },
    ));
    let error = compiler
        .compile_with(
            "ВЫБРАТЬ Ссылка ИЗ Справочник.Товары",
            &CompileOptions::new().session(&session),
        )
        .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::Metadata);
    assert!(!error.is_missing_session_parameter());
}
#[test]
fn disabled_independent_postgres() {
    disabled(PostgresBackend);
}
#[test]
fn disabled_independent_mssql() {
    disabled(MsSqlBackend::new(2000).unwrap());
}

fn lazy<B: Backend>(backend: B) {
    let snapshot = support::separators_snapshot();
    let compiler = QueryCompiler::new(&snapshot, backend);
    let source = "ВЫБРАТЬ Ссылка ИЗ Справочник.Товары";
    let missing = compiler.compile(source).unwrap_err();
    assert_eq!(
        missing.missing_parameter_name(),
        Some("ИспользованиеРазделителя")
    );
    assert!(missing.is_missing_session_parameter());
    let mut session = SessionParameters::new();
    session.set(QueryParameter::new(
        "ИспользованиеРазделителя",
        ParameterValue::Boolean(true),
    ));
    let missing = compiler
        .compile_with(source, &CompileOptions::new().session(&session))
        .unwrap_err();
    assert_eq!(
        missing.missing_parameter_name(),
        Some("ЗначениеРазделителя")
    );
    assert!(missing.is_missing_session_parameter());
    assert!(compiler.compile("ВЫБРАТЬ 1 КАК Число").is_ok());
    assert!(
        compiler
            .compile("ВЫБРАТЬ КОЛИЧЕСТВО(*) КАК Н ИЗ Константы")
            .is_ok()
    );
    assert!(compiler.prepare(source).is_ok());
    let mut value_only = SessionParameters::new();
    value_only.set(QueryParameter::new(
        "ЗначениеРазделителя",
        ParameterValue::Number {
            unscaled: 7,
            scale: 0,
        },
    ));
    let missing = compiler
        .compile_with(source, &CompileOptions::new().session(&value_only))
        .unwrap_err();
    assert_eq!(
        missing.missing_parameter_name(),
        Some("ИспользованиеРазделителя")
    );

    for text in [
        "ВЫБРАТЬ Значение ИЗ Константа.ОсновнойТовар",
        "ВЫБРАТЬ ОсновнойТовар ИЗ Константы",
        "ВЫБРАТЬ Поставщик.Артикул ИЗ Справочник.Товары",
    ] {
        let missing = compiler.compile(text).unwrap_err();
        assert_eq!(
            missing.missing_parameter_name(),
            Some("ИспользованиеРазделителя"),
            "{text}: {missing}"
        );
        session.set(QueryParameter::new(
            "ИспользованиеРазделителя",
            ParameterValue::Boolean(false),
        ));
        let error = compiler
            .compile_with(text, &CompileOptions::new().session(&session))
            .unwrap_err();
        assert_eq!(
            error.kind(),
            QueryDiagnosticKind::Metadata,
            "{text}: {error}"
        );
        assert!(!error.is_missing_session_parameter());
    }
}
#[test]
fn lazy_parameters_postgres() {
    lazy(PostgresBackend);
}
#[test]
fn lazy_parameters_mssql() {
    lazy(MsSqlBackend::new(2000).unwrap());
}

#[test]
fn disabled_shared_reads_without_a_value_or_predicate() {
    use open_sdbl::metadata::SeparatedDataUse;
    let snapshot = support::with_descriptors(support::separators_snapshot(), |descriptors| {
        for descriptor in descriptors {
            if let Some(separation) = &mut descriptor.separation {
                separation.mode = SeparatedDataUse::IndependentAndShared;
            }
        }
    });
    let mut session = SessionParameters::new();
    session.set(QueryParameter::new(
        "ИспользованиеРазделителя",
        ParameterValue::Boolean(false),
    ));
    let source = "ВЫБРАТЬ Ссылка ИЗ Справочник.Товары";
    let pg = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile_with(source, &CompileOptions::new().session(&session))
        .unwrap();
    assert_eq!(
        pg.sql,
        "SELECT \"__src\".\"_idrref\" AS \"Ссылка\" FROM \"_reference53\" AS \"__src\""
    );
    let ms = QueryCompiler::new(&snapshot, MsSqlBackend::new(2000).unwrap())
        .compile_with(source, &CompileOptions::new().session(&session))
        .unwrap();
    assert_eq!(
        ms.sql,
        "SELECT [__src].[_idrref] AS [Ссылка] FROM [_reference53] AS [__src]"
    );
}

#[test]
fn a_read_table_without_separator_columns_does_not_request_other_tables_values() {
    let snapshot = support::with_live_tables(support::separators_snapshot(), |tables| {
        for table in tables
            .iter_mut()
            .filter(|table| table.name.eq_ignore_ascii_case("_const61"))
        {
            table.columns.retain(|column| {
                !["_fld56", "_fld57"]
                    .iter()
                    .any(|name| column.name.eq_ignore_ascii_case(name))
            });
        }
    });
    let source = "ВЫБРАТЬ Значение ИЗ Константа.ОсновнойТовар";
    assert!(
        QueryCompiler::new(&snapshot, PostgresBackend)
            .compile(source)
            .is_ok()
    );
    assert!(
        QueryCompiler::new(&snapshot, MsSqlBackend::new(2000).unwrap())
            .compile(source)
            .is_ok()
    );
    let source = "ВЫБРАТЬ ОсновнойТовар.Артикул ИЗ Константы";
    let missing = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile(source)
        .unwrap_err();
    assert_eq!(
        missing.missing_parameter_name(),
        Some("ИспользованиеРазделителя")
    );
    let missing = QueryCompiler::new(&snapshot, MsSqlBackend::new(2000).unwrap())
        .compile(source)
        .unwrap_err();
    assert_eq!(
        missing.missing_parameter_name(),
        Some("ИспользованиеРазделителя")
    );
    let mut session = SessionParameters::new();
    session.set(QueryParameter::new(
        "ИспользованиеРазделителя",
        ParameterValue::Boolean(false),
    ));
    let error = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile_with(source, &CompileOptions::new().session(&session))
        .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::Metadata);
    let error = QueryCompiler::new(&snapshot, MsSqlBackend::new(2000).unwrap())
        .compile_with(source, &CompileOptions::new().session(&session))
        .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::Metadata);
}
