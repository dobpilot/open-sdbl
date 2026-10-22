//! Структурный запрос уже связанного значения Shared, без предположений о ленивом Session.
mod support;
use open_sdbl::metadata::{MetadataSnapshot, SeparatedDataUse};
use open_sdbl::query::{
    Backend, CompileOptions, MsSqlBackend, ParameterValue, PostgresBackend, QueryCompiler,
    QueryParameter, SessionParameters,
};
const SOURCE: &str = "ВЫБРАТЬ Ссылка ИЗ Справочник.Товары";
const CONDITION: &str = "cc906f05-5ea6-43cb-a084-c4ee51ca4bd5";
fn snapshot(string: bool, use_bound: bool, value_bound: bool) -> MetadataSnapshot {
    let base = support::with_descriptors(support::separators_snapshot(), |descriptors| {
        for descriptor in descriptors {
            if let Some(settings) = &mut descriptor.separation {
                settings.mode = SeparatedDataUse::IndependentAndShared;
                settings.conditional_separation = Some(support::guid(CONDITION));
                if !use_bound {
                    settings.use_parameter = None;
                }
                if !value_bound {
                    settings.value_parameter = None;
                }
            }
        }
    });
    let mut schema = base.schema().clone();
    if string {
        for table in &mut schema.tables {
            for column in &mut table.columns {
                if column.name == "Fld56" || column.name == "Fld57" {
                    for kind in &mut column.types {
                        kind.tag = "S".to_owned();
                    }
                }
            }
        }
    }
    open_sdbl::metadata::resolve_metadata(
        base.db_names().clone(),
        base.descriptors().to_vec(),
        schema,
        base.live_tables().to_vec(),
    )
    .snapshot
}
fn control<B: Backend + Copy>(backend: B, string: bool) {
    let snapshot = snapshot(string, true, true);
    let compiler = QueryCompiler::new(&snapshot, backend);
    assert!(compiler.prepare(SOURCE).is_ok());
    let mut session = SessionParameters::new();
    session.set_separation_condition(support::guid(CONDITION), true);
    session.set(QueryParameter::new(
        "ИспользованиеРазделителя",
        ParameterValue::Boolean(true),
    ));
    let missing = compiler
        .compile_with(SOURCE, &CompileOptions::new().session(&session))
        .unwrap_err();
    assert!(missing.is_missing_session_parameter());
    assert_eq!(
        missing.missing_parameter_name(),
        Some("ЗначениеРазделителя")
    );
    let value = if string {
        ParameterValue::String("B".to_owned())
    } else {
        ParameterValue::Number {
            unscaled: 2,
            scale: 0,
        }
    };
    session.set(QueryParameter::new("ЗначениеРазделителя", value));
    let compiled = compiler
        .compile_with(SOURCE, &CompileOptions::new().session(&session))
        .unwrap();
    assert!(
        compiled.sql.contains(if string { "'B'" } else { " = 2" }),
        "{}",
        compiled.sql
    );
    let mut inactive = SessionParameters::new();
    inactive.set_separation_condition(support::guid(CONDITION), false);
    assert!(
        compiler
            .compile_with(SOURCE, &CompileOptions::new().session(&inactive))
            .is_ok()
    );
    inactive.set_separation_condition(support::guid(CONDITION), true);
    inactive.set(QueryParameter::new(
        "ИспользованиеРазделителя",
        ParameterValue::Boolean(false),
    ));
    let disabled = compiler
        .compile_with(SOURCE, &CompileOptions::new().session(&inactive))
        .unwrap();
    assert!(!disabled.sql.contains("WHERE"), "{}", disabled.sql);
    assert!(compiler.compile("ВЫБРАТЬ 1 КАК Число").is_ok());
    for (use_bound, value_bound) in [(false, true), (true, false), (false, false)] {
        let fallback = self::snapshot(string, use_bound, value_bound);
        let compiler = QueryCompiler::new(&fallback, backend);
        let mut session = SessionParameters::new();
        session.set_separation_condition(support::guid(CONDITION), true);
        session.set(QueryParameter::new(
            "ИспользованиеРазделителя",
            ParameterValue::Boolean(true),
        ));
        assert!(
            compiler
                .compile_with(SOURCE, &CompileOptions::new().session(&session))
                .is_ok()
        );
    }
}
#[test]
fn string_postgres() {
    control(PostgresBackend, true);
}
#[test]
fn number_postgres() {
    control(PostgresBackend, false);
}
#[test]
fn string_mssql() {
    control(MsSqlBackend::new(2000).unwrap(), true);
}
#[test]
fn number_mssql() {
    control(MsSqlBackend::new(2000).unwrap(), false);
}
