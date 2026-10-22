mod support;
use open_sdbl::query::{LogicalResultValue, PostgresBackend, QueryCompiler};
use support::*;

#[test]
fn scalar_occurrences_keep_full_aliases_and_independent_indices() {
    let snapshot = snapshot();
    let query = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile("ВЫБРАТЬ Т.Code КАК Первое, Т.Code КАК Второе ИЗ Справочник.OpenSdblMetadataProbe КАК Т")
        .unwrap();
    let plan = query.logical_projection.as_ref().unwrap();
    assert_eq!(plan.len(), 2);
    assert_eq!(plan[0].name, "Первое");
    assert_eq!(plan[1].name, "Второе");
    assert_eq!(plan[0].value, LogicalResultValue::Scalar { column: 0 });
    assert_eq!(plan[1].value, LogicalResultValue::Scalar { column: 1 });
}

fn boolean_string_snapshot() -> open_sdbl::metadata::MetadataSnapshot {
    let base = mixed_composite_snapshot();
    let mut schema = base.schema().clone();
    let field = schema
        .tables
        .iter_mut()
        .find(|table| table.name == "Reference53")
        .unwrap()
        .columns
        .iter_mut()
        .find(|column| column.name == "Fld54")
        .unwrap();
    field.types[1].tag = "L".to_owned();
    field.types[1].reference_target = None;
    let mut tables = base.live_tables().to_vec();
    let member = tables[0]
        .columns
        .iter_mut()
        .find(|column| column.name == "_fld54_rrref")
        .unwrap();
    member.name = "_fld54_l".to_owned();
    member.data_type = "boolean".to_owned();
    open_sdbl::metadata::resolve_metadata(
        base.db_names().clone(),
        base.descriptors().to_vec(),
        schema,
        tables,
    )
    .snapshot
}

#[test]
fn boolean_string_field_and_case_are_single_occurrences() {
    let snapshot = boolean_string_snapshot();
    for source in [
        "SELECT ProbeAttribute AS Value FROM Catalog.OpenSdblMetadataProbe",
        "SELECT CASE WHEN TRUE THEN FALSE ELSE \"x\" END AS Value FROM Catalog.OpenSdblMetadataProbe",
        "SELECT CASE WHEN TRUE THEN FALSE ELSE \"x\" END AS Value FROM Catalog.OpenSdblMetadataProbe UNION ALL SELECT CASE WHEN FALSE THEN TRUE ELSE \"y\" END AS Other FROM Catalog.OpenSdblMetadataProbe",
    ] {
        let query = QueryCompiler::new(&snapshot, PostgresBackend)
            .compile(source)
            .unwrap();
        let plan = query.logical_projection.as_ref().unwrap();
        assert_eq!(plan.len(), 1, "{source}");
        assert_eq!(plan[0].name, "Value");
        let LogicalResultValue::Composite {
            discriminator,
            members,
        } = &plan[0].value
        else {
            panic!("{source}")
        };
        assert!(matches!(
            query.columns[*discriminator].kind,
            open_sdbl::query::ColumnKind::Binary { .. }
        ));
        assert_eq!(
            members
                .iter()
                .filter(|member| member.column.is_some())
                .count(),
            2
        );
        for member in members {
            if member.type_tag == open_sdbl::query::TypeValue::Undefined.tag() {
                assert!(member.column.is_none());
                continue;
            }
            let kind = &query.columns[member.column.unwrap()].kind;
            match member.type_tag {
                2 => assert_eq!(*kind, open_sdbl::query::ColumnKind::Boolean),
                5 => assert!(matches!(kind, open_sdbl::query::ColumnKind::String { .. })),
                _ => panic!("unknown tag"),
            }
        }
    }
}

#[test]
fn accepted_union_spreads_scalars_into_one_final_recipe() {
    let snapshot = snapshot();
    let query = QueryCompiler::new(&snapshot, PostgresBackend).compile("SELECT FALSE AS Value FROM Catalog.OpenSdblMetadataProbe UNION ALL SELECT \"x\" FROM Catalog.OpenSdblMetadataProbe").unwrap();
    let plan = query.logical_projection.unwrap();
    assert_eq!(plan.len(), 1);
    assert!(matches!(
        plan[0].value,
        LogicalResultValue::Composite { .. }
    ));
}

#[test]
fn long_aliases_survive_physical_label_truncation() {
    let snapshot = snapshot();
    let first = "Первое".repeat(30);
    let second = format!("{first}Второе");
    let source =
        format!("SELECT Code AS {first}, Code AS {second} FROM Catalog.OpenSdblMetadataProbe");
    let query = QueryCompiler::new(&snapshot, open_sdbl::query::MsSqlBackend::new(0).unwrap())
        .compile(&source)
        .unwrap();
    let plan = query.logical_projection.unwrap();
    assert_eq!(plan[0].name, first);
    assert_eq!(plan[1].name, second);
    assert_ne!(query.columns[0].label, plan[0].name);
    assert_ne!(query.columns[0].label, query.columns[1].label);
}

#[test]
fn composite_reference_uses_the_emitted_payload_cell() {
    let snapshot = mixed_composite_snapshot();
    let query = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile(
            "SELECT ProbeAttribute AS A, ProbeAttribute AS B FROM Catalog.OpenSdblMetadataProbe",
        )
        .unwrap();
    let plan = query.logical_projection.unwrap();
    assert_eq!(plan.len(), 2);
    for (ordinal, item) in plan.iter().enumerate() {
        let LogicalResultValue::Composite {
            discriminator,
            members,
        } = &item.value
        else {
            panic!()
        };
        assert!(*discriminator >= ordinal * 3 && *discriminator < (ordinal + 1) * 3);
        let member = members.iter().find(|member| member.type_tag == 8).unwrap();
        assert!(matches!(
            query.columns[member.column.unwrap()].kind,
            open_sdbl::query::ColumnKind::Reference { .. }
        ));
    }
}

#[test]
fn unknown_valueless_composite_and_special_results_are_unavailable() {
    let snapshot = snapshot();
    for source in [
        "SELECT CASE WHEN TRUE THEN UNDEFINED WHEN FALSE THEN Ссылка ELSE \"x\" END AS Value FROM Catalog.OpenSdblMetadataProbe",
        "SELECT X.Code FROM (SELECT Code FROM Catalog.OpenSdblMetadataProbe) AS X",
    ] {
        let query = QueryCompiler::new(&snapshot, PostgresBackend)
            .compile(source)
            .unwrap();
        assert!(query.logical_projection.is_none(), "{source}");
    }
    let snapshot = tabular_section_snapshot();
    let query = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile("ВЫБРАТЬ Ссылка, ГрафикНачислений ИЗ Документ.бит_ДополнительныеУсловияПоДоговору")
        .unwrap();
    assert!(query.logical_projection.is_none());
}

#[test]
fn an_invalid_discriminator_catalog_kind_is_unavailable() {
    let snapshot = with_live_tables(boolean_string_snapshot(), |tables| {
        let member = tables[0]
            .columns
            .iter_mut()
            .find(|column| column.name == "_fld54_type")
            .unwrap();
        member.data_type = "mvarchar(10)".to_owned();
    });
    let query = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile("SELECT ProbeAttribute FROM Catalog.OpenSdblMetadataProbe")
        .unwrap();
    assert!(query.logical_projection.is_none());
}

#[test]
fn untagged_undefined_coercion_is_unavailable_without_changing_sql() {
    let snapshot = snapshot();
    for source in [
        "SELECT UNDEFINED AS Value FROM Catalog.OpenSdblMetadataProbe UNION ALL SELECT \"x\" FROM Catalog.OpenSdblMetadataProbe",
        "SELECT CASE WHEN TRUE THEN UNDEFINED ELSE \"x\" END AS Value FROM Catalog.OpenSdblMetadataProbe",
    ] {
        let query = QueryCompiler::new(&snapshot, PostgresBackend)
            .compile(source)
            .unwrap();
        assert!(query.logical_projection.is_none());
    }
    let query = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile("SELECT UNDEFINED AS Value")
        .unwrap();
    assert!(query.logical_projection.is_some());
}

#[test]
fn binary_null_tag_alternative_is_unavailable() {
    let snapshot = snapshot();
    let parameters = [open_sdbl::query::QueryParameter::new(
        "Payload",
        open_sdbl::query::ParameterValue::Binary(vec![1, 2]),
    )];
    let options = open_sdbl::query::CompileOptions::new().parameters(&parameters);
    let query = QueryCompiler::new(&snapshot, PostgresBackend).compile_with("SELECT CASE WHEN TRUE THEN &Payload ELSE \"x\" END AS Value FROM Catalog.OpenSdblMetadataProbe", &options).unwrap();
    assert!(query.logical_projection.is_none());
}

#[test]
fn source_free_projection_refuses_lost_undefined_alternatives() {
    let snapshot = snapshot();
    for source in [
        "SELECT CASE WHEN TRUE THEN UNDEFINED ELSE \"x\" END AS Value",
        "SELECT 1 AS Kept, CASE WHEN FALSE THEN \"x\" ELSE UNDEFINED END AS Lost",
        "SELECT CASE WHEN TRUE THEN UNDEFINED ELSE \"x\" END AS Value UNION ALL SELECT \"y\"",
    ] {
        let query = QueryCompiler::new(&snapshot, PostgresBackend)
            .compile(source)
            .unwrap();
        assert!(query.logical_projection.is_none(), "{source}");
    }
    let query = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile("SELECT CASE WHEN TRUE THEN UNDEFINED ELSE \"x\" END AS Value")
        .unwrap();
    assert_eq!(
        query.sql,
        "SELECT CASE WHEN TRUE THEN NULL ELSE 'x' END AS \"Value\""
    );
}

#[test]
fn source_free_exact_scalar_and_null_recipes_remain_available() {
    let snapshot = snapshot();
    for expression in [
        "UNDEFINED",
        "NULL",
        "17",
        "\"x\"",
        "CASE WHEN TRUE THEN NULL ELSE \"x\" END",
    ] {
        let query = QueryCompiler::new(&snapshot, PostgresBackend)
            .compile(&format!("SELECT {expression} AS Value"))
            .unwrap();
        let recipe = query.logical_projection.unwrap();
        assert_eq!(recipe.len(), 1);
        assert_eq!(recipe[0].name, "Value");
        assert_eq!(recipe[0].value, LogicalResultValue::Scalar { column: 0 });
    }
}
