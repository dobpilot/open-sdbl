mod support;

use open_sdbl::query::{
    CompileOptions, ParameterValue, PostgresBackend, QueryCompiler, QueryParameter,
};

#[test]
fn native_string_equality_trims_only_trailing_spaces_and_keeps_case() {
    let snapshot = support::snapshot();
    for value in ["AbC", "abc", "AbC "] {
        let params = [QueryParameter::new(
            "Value",
            ParameterValue::String(value.into()),
        )];
        let query = QueryCompiler::new(&snapshot, PostgresBackend)
            .compile_with(
                "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE Code = &Value",
                &CompileOptions::new().parameters(&params),
            )
            .unwrap();
        assert!(query.sql.contains("rtrim(CAST("), "{}", query.sql);
        assert!(query.sql.contains("COLLATE \"C\""), "{}", query.sql);
    }
}

#[test]
fn incompatible_number_and_boolean_do_not_cast_to_the_string_field() {
    let snapshot = support::snapshot();
    for value in [
        ParameterValue::Number {
            unscaled: 17,
            scale: 0,
        },
        ParameterValue::Boolean(true),
    ] {
        let params = [QueryParameter::new("Value", value)];
        let query = QueryCompiler::new(&snapshot, PostgresBackend)
            .compile_with(
                "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE Code = &Value",
                &CompileOptions::new().parameters(&params),
            )
            .unwrap();
        assert!(query.sql.contains("ELSE FALSE END"), "{}", query.sql);
        assert!(query.sql.contains("IS NULL"), "{}", query.sql);
        assert!(query.sql.contains("_code"), "{}", query.sql);
    }
}

#[test]
fn literal_and_reversed_operands_use_the_same_measured_path() {
    let snapshot = support::snapshot();
    for predicate in ["Code = 17", "17 = Code", "Code = TRUE", "FALSE = Code"] {
        let query = QueryCompiler::new(&snapshot, PostgresBackend)
            .compile(&format!(
                "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE {predicate}"
            ))
            .unwrap();
        assert!(query.sql.contains("ELSE FALSE END"), "{}", query.sql);
    }
}

#[test]
fn sql_server_and_inequality_keep_the_existing_path() {
    use open_sdbl::query::MsSqlBackend;
    let snapshot = support::snapshot();
    let source = "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE Code = \"AbC \"";
    let query = QueryCompiler::new(&snapshot, MsSqlBackend::new(0).unwrap())
        .compile(source)
        .unwrap();
    assert!(!query.sql.contains("COLLATE"), "{}", query.sql);
    assert!(!query.sql.contains("rtrim"), "{}", query.sql);
    let query = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile("SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE Code <> \"AbC\"")
        .unwrap();
    assert!(!query.sql.contains("COLLATE"), "{}", query.sql);
}

#[test]
fn native_date_field_compared_with_a_string_is_not_a_provider_cast() {
    let snapshot = support::snapshot();
    let query = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile("SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE Date = \"2024-01-01\"")
        .unwrap();
    assert!(query.sql.contains("ELSE FALSE END"), "{}", query.sql);
}

#[test]
fn contextual_mixed_case_comparison_keeps_its_existing_type_selection() {
    let snapshot = support::snapshot();
    let query = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile("SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE CASE WHEN Date > DATETIME(2020, 1, 1) THEN Code ELSE FALSE END = \"A\"")
        .unwrap();
    assert!(query.sql.contains("ELSE NULL END = 'A'"), "{}", query.sql);
}

#[test]
fn source_free_equality_matches_sourced_scalar_equality() {
    let snapshot = support::snapshot();
    for expression in [
        "\"AbC\" = \"AbC \"",
        "\"AbC\" = \"abc\"",
        "\"x\" = 17",
        "TRUE = \"x\"",
        "\"x\" = DATETIME(2020, 1, 1)",
        "1 + 1 = \"x\"",
        "\"x\" = 1 + 1",
        "CAST(NULL AS NUMBER) = \"x\"",
    ] {
        let compiler = QueryCompiler::new(&snapshot, PostgresBackend);
        let free = compiler
            .compile(&format!("SELECT {expression} AS Value"))
            .unwrap();
        let sourced = compiler
            .compile(&format!(
                "SELECT {expression} AS Value FROM Catalog.OpenSdblMetadataProbe"
            ))
            .unwrap();
        assert!(
            sourced.sql.starts_with(&free.sql),
            "{expression}: {} != {}",
            free.sql,
            sourced.sql
        );
        assert!(
            free.sql.contains("rtrim(CAST(") || free.sql.contains("ELSE FALSE END"),
            "{}",
            free.sql
        );
    }
}

#[test]
fn equality_uses_complete_binary_operands_in_both_orders() {
    let snapshot = support::snapshot();
    for predicate in ["1 + 1 = Code", "Code = 1 + 1", "2 * 3 + 1 = Code"] {
        let query = QueryCompiler::new(&snapshot, PostgresBackend)
            .compile(&format!(
                "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE {predicate}"
            ))
            .unwrap();
        assert!(query.sql.contains("ELSE FALSE END"), "{}", query.sql);
        assert!(query.sql.contains("IS NULL"), "{}", query.sql);
    }
    for predicate in ["Code + \"\" = \"AbC \"", "\"AbC \" = Code + \"\""] {
        let query = QueryCompiler::new(&snapshot, PostgresBackend)
            .compile(&format!(
                "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE {predicate}"
            ))
            .unwrap();
        assert_eq!(query.sql.matches("rtrim(CAST(").count(), 2, "{}", query.sql);
        assert!(query.sql.contains(" || "), "{}", query.sql);
    }
}

#[test]
fn source_free_conjunction_keeps_each_comparison_policy() {
    let snapshot = support::snapshot();
    for source in [
        "SELECT 1 WHERE \"a\" = \"a \" AND \"b\" = \"b \"",
        "SELECT \"a\" = \"a \" AND \"b\" = \"b \" AS Value",
    ] {
        let query = QueryCompiler::new(&snapshot, PostgresBackend)
            .compile(source)
            .unwrap();
        assert_eq!(query.sql.matches("rtrim(CAST(").count(), 4, "{}", query.sql);
    }
}
