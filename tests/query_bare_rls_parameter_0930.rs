//! Контекст прямого параметра RLS по нативному замеру 1С 8.3.27.2342.
mod support;

use open_sdbl::metadata::ObjectId;
use open_sdbl::query::{
    AccessRestriction, Backend, CompileOptions, MsSqlBackend, ParameterValue, PostgresBackend,
    QueryCompiler, QueryParameter, SessionParameters, find_metadata_object,
};

fn check<B: Backend + Copy>(backend: B) {
    let snapshot = support::accumulation_register_snapshot();
    let object = find_metadata_object(&snapshot, "AccumulationRegister.Остатки").unwrap();
    let restrictions = [AccessRestriction::new(
        ObjectId::from(&object.guid),
        "ГДЕ ИСТИНА = &ДоступнаОбласть",
    )];
    let compiler = QueryCompiler::new(&snapshot, backend);
    let source = "SELECT ALLOWED Номенклатура FROM AccumulationRegister.Остатки.Balance()";
    let prepared = compiler.prepare(source).unwrap();
    let missing = prepared
        .compile_with(
            &snapshot,
            &CompileOptions::new().restrictions(&restrictions),
        )
        .unwrap_err();
    assert_eq!(missing.missing_parameter_name(), Some("ДоступнаОбласть"));
    assert!(missing.is_missing_session_parameter(), "{missing}");
    let mut session = SessionParameters::new();
    for value in [true, false] {
        session.set(QueryParameter::new(
            "ДоступнаОбласть",
            ParameterValue::Boolean(value),
        ));
        prepared
            .compile_with(
                &snapshot,
                &CompileOptions::new()
                    .restrictions(&restrictions)
                    .session(&session),
            )
            .unwrap();
    }
    let absent = compiler
        .compile_with("SELECT &Обычный", &CompileOptions::new())
        .unwrap_err();
    assert_eq!(absent.missing_parameter_name(), Some("Обычный"));
    assert!(!absent.is_missing_session_parameter());
    let query_values = [QueryParameter::new(
        "ДоступнаОбласть",
        ParameterValue::Boolean(true),
    )];
    let absent = compiler.compile_with(
        "SELECT ALLOWED Номенклатура FROM AccumulationRegister.Остатки.Balance() WHERE &ДоступнаОбласть = TRUE",
        &CompileOptions::new().restrictions(&restrictions).parameters(&query_values),
    ).unwrap_err();
    assert!(absent.is_missing_session_parameter(), "{absent}");
    assert_eq!(absent.missing_parameter_name(), Some("ДоступнаОбласть"));
}

#[test]
fn direct_rls_parameter_preserves_its_origin_on_both_backends() {
    check(PostgresBackend);
    check(MsSqlBackend::new(0).unwrap());
}
