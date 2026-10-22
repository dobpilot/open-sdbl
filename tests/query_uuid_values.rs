mod support;

use open_sdbl::metadata::{ConfigDeclaredType, MetadataSnapshot, resolve_metadata};
use open_sdbl::query::{ColumnKind, PostgresBackend, QueryCompiler};

fn uuid_snapshot() -> MetadataSnapshot {
    let base = support::snapshot();
    let mut descriptors = base.descriptors().to_vec();
    let attribute = descriptors
        .iter_mut()
        .find(|d| d.name == "ProbeAttribute")
        .unwrap();
    attribute.declared_types = Some(vec![ConfigDeclaredType {
        tag: "#".into(),
        reference: Some(support::guid("fc01b5df-97fe-449b-83d4-218a090e681e")),
        qualifiers: vec![],
    }]);
    resolve_metadata(
        base.db_names().clone(),
        descriptors,
        base.schema().clone(),
        base.live_tables().to_vec(),
    )
    .snapshot
}

#[test]
fn a_native_uuid_attribute_is_not_an_arbitrary_binary_column() {
    let snapshot = uuid_snapshot();
    let query = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile("SELECT ProbeAttribute FROM Catalog.OpenSdblMetadataProbe")
        .unwrap();
    assert_eq!(query.columns[0].kind, ColumnKind::Uuid);
    assert!(query.sql.contains("::uuid"), "{}", query.sql);
}

#[test]
fn unknown_binary_declarations_keep_their_existing_kind() {
    let snapshot = support::snapshot();
    let query = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile("SELECT ProbeAttribute FROM Catalog.OpenSdblMetadataProbe")
        .unwrap();
    assert!(matches!(query.columns[0].kind, ColumnKind::Binary { .. }));
}

#[test]
fn typed_parameters_keep_uuid_kind_and_existing_byte_order() {
    use open_sdbl::query::{CompileOptions, ParameterValue, QueryParameter};
    let snapshot = uuid_snapshot();
    for id in [
        "00112233-4455-6677-8899-aabbccddeeff",
        "00000000-0000-0000-0000-000000000000",
    ] {
        let parameters = [QueryParameter::new(
            "Id",
            ParameterValue::Uuid(support::guid(id)),
        )];
        let query = QueryCompiler::new(&snapshot, PostgresBackend)
            .compile_with(
                "SELECT &Id AS Id",
                &CompileOptions::new().parameters(&parameters),
            )
            .unwrap();
        assert_eq!(query.columns[0].kind, ColumnKind::Uuid);
        assert!(query.sql.contains("::uuid"));
    }
}

#[test]
fn derived_and_union_columns_preserve_uuid() {
    use open_sdbl::query::{CompileOptions, ParameterValue, QueryParameter};
    let snapshot = uuid_snapshot();
    let parameters = [QueryParameter::new(
        "Id",
        ParameterValue::Uuid(support::guid("00112233-4455-6677-8899-aabbccddeeff")),
    )];
    for text in [
        "SELECT ProbeAttribute AS Id FROM Catalog.OpenSdblMetadataProbe WHERE ProbeAttribute = &Id UNION ALL SELECT ProbeAttribute FROM Catalog.OpenSdblMetadataProbe WHERE ProbeAttribute = &Id",
        "SELECT q.Id FROM (SELECT ProbeAttribute AS Id FROM Catalog.OpenSdblMetadataProbe) AS q WHERE q.Id = &Id",
        "SELECT q.Id FROM (SELECT &Id AS Id) AS q",
    ] {
        let query = QueryCompiler::new(&snapshot, PostgresBackend)
            .compile_with(text, &CompileOptions::new().parameters(&parameters))
            .unwrap();
        assert_eq!(query.columns[0].kind, ColumnKind::Uuid);
    }
}

#[test]
fn raw_binary_parameters_are_not_uuid_parameters() {
    use open_sdbl::query::{CompileOptions, ParameterValue, QueryParameter};
    let snapshot = uuid_snapshot();
    let parameters = [QueryParameter::new(
        "Bytes",
        ParameterValue::Binary(vec![0; 16]),
    )];
    let query = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile_with(
            "SELECT &Bytes",
            &CompileOptions::new().parameters(&parameters),
        )
        .unwrap();
    assert!(matches!(query.columns[0].kind, ColumnKind::Binary { .. }));
}

#[test]
fn ambiguous_or_qualified_declarations_do_not_reclassify_binary() {
    use open_sdbl::metadata::Value;
    let base = uuid_snapshot();
    for variant in 0..3 {
        let mut descriptors = base.descriptors().to_vec();
        let pos = descriptors
            .iter()
            .position(|d| d.name == "ProbeAttribute")
            .unwrap();
        match variant {
            0 => {
                let copy = descriptors[pos].clone();
                descriptors.push(copy);
            }
            1 => descriptors[pos]
                .declared_types
                .as_mut()
                .unwrap()
                .push(ConfigDeclaredType {
                    tag: "S".into(),
                    reference: None,
                    qualifiers: vec![],
                }),
            _ => descriptors[pos].declared_types.as_mut().unwrap()[0]
                .qualifiers
                .push(Value::Atom("0".into())),
        }
        let snapshot = resolve_metadata(
            base.db_names().clone(),
            descriptors,
            base.schema().clone(),
            base.live_tables().to_vec(),
        )
        .snapshot;
        let query = QueryCompiler::new(&snapshot, PostgresBackend)
            .compile("SELECT ProbeAttribute FROM Catalog.OpenSdblMetadataProbe")
            .unwrap();
        assert!(matches!(query.columns[0].kind, ColumnKind::Binary { .. }));
    }
}

#[test]
fn the_new_uuid_forms_reject_unmeasured_sql_server() {
    use open_sdbl::query::{
        CompileOptions, MsSqlBackend, ParameterValue, QueryDiagnosticKind, QueryParameter,
    };
    let snapshot = uuid_snapshot();
    let compiler = QueryCompiler::new(&snapshot, MsSqlBackend::new(0).unwrap());
    let parameters = [QueryParameter::new(
        "Id",
        ParameterValue::Uuid(support::guid("00112233-4455-6677-8899-aabbccddeeff")),
    )];
    assert_eq!(
        compiler
            .compile_with("SELECT &Id", &CompileOptions::new().parameters(&parameters))
            .unwrap_err()
            .kind(),
        QueryDiagnosticKind::UnsupportedFeature
    );
    assert_eq!(
        compiler
            .compile("SELECT ProbeAttribute FROM Catalog.OpenSdblMetadataProbe")
            .unwrap_err()
            .kind(),
        QueryDiagnosticKind::UnsupportedFeature
    );
}
