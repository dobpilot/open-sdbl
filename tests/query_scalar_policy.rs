mod support;
use open_sdbl::query::{Backend, MsSqlBackend, PostgresBackend, QueryCompiler};

fn projection_parity(backend: impl Backend) {
    let snapshot = support::snapshot();
    for (expression, exact) in [
        ("NULL", true),
        ("UNDEFINED", true),
        ("\"x\"", true),
        ("CASE WHEN TRUE THEN NULL ELSE \"x\" END", true),
        ("CASE WHEN TRUE THEN UNDEFINED ELSE \"x\" END", false),
        ("CASE WHEN FALSE THEN \"x\" ELSE UNDEFINED END", false),
    ] {
        for suffix in ["", " FROM Catalog.OpenSdblMetadataProbe"] {
            let source = format!("SELECT {expression} AS Value{suffix}");
            let query = QueryCompiler::new(&snapshot, backend)
                .compile(&source)
                .unwrap();
            assert_eq!(query.logical_projection.is_some(), exact, "{source}");
        }
    }
}

#[test]
fn both_contexts_share_exact_projection_policy_in_both_dialects() {
    projection_parity(PostgresBackend);
    projection_parity(MsSqlBackend::new(0).unwrap());
}

fn comparison_boundaries(backend: impl Backend) {
    let snapshot = support::snapshot();
    for expression in [
        "\"a\" <> \"a \"",
        "\"a\" = NULL",
        "CASE WHEN TRUE THEN \"a\" ELSE \"b\" END = \"a \"",
        "\"a \" = CASE WHEN TRUE THEN \"a\" ELSE \"b\" END",
    ] {
        for suffix in ["", " FROM Catalog.OpenSdblMetadataProbe"] {
            let source = format!("SELECT {expression} AS Value{suffix}");
            let query = QueryCompiler::new(&snapshot, backend)
                .compile(&source)
                .unwrap();
            assert!(!query.sql.contains("rtrim("), "{}", query.sql);
            assert!(!query.sql.contains("COLLATE"), "{}", query.sql);
        }
    }
}

#[test]
fn contextual_case_null_and_inequality_keep_existing_paths() {
    comparison_boundaries(PostgresBackend);
    comparison_boundaries(MsSqlBackend::new(0).unwrap());
}

#[test]
fn sql_server_scalar_equality_keeps_provider_semantics_in_both_contexts() {
    let snapshot = support::snapshot();
    for expression in ["\"a\" = \"a \"", "\"a\" = 17"] {
        for suffix in ["", " FROM Catalog.OpenSdblMetadataProbe"] {
            let query = QueryCompiler::new(&snapshot, MsSqlBackend::new(0).unwrap())
                .compile(&format!("SELECT {expression} AS Value{suffix}"))
                .unwrap();
            assert!(!query.sql.contains("rtrim("), "{}", query.sql);
            assert!(!query.sql.contains("COLLATE"), "{}", query.sql);
            assert!(!query.sql.contains("ELSE FALSE END"), "{}", query.sql);
        }
    }
}
