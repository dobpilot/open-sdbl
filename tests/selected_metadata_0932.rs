mod support;
use open_sdbl::metadata::{DbNames, MetadataKind, ResolutionFinding, resolve_selected_metadata};
use open_sdbl::query::{
    CompileOptions, MsSqlBackend, ParameterValue, PostgresBackend, QueryCompiler, QueryParameter,
    SessionParameters,
};

#[test]
fn selected_query_mappings_match_full_resolution_and_omit_unreached_objects() {
    let full = support::reference_snapshot();
    let owner = full
        .object_id(MetadataKind::Catalog, "OpenSdblMetadataProbe")
        .unwrap();
    let descriptors = full
        .descriptors()
        .iter()
        .filter(|descriptor| descriptor.resource_guid.to_bytes() == owner.as_bytes())
        .cloned()
        .collect();
    let selected = resolve_selected_metadata(
        full.db_names(),
        descriptors,
        vec![],
        vec![],
        full.schema(),
        full.live_tables(),
    )
    .snapshot;
    assert!(selected.objects().len() < full.objects().len());
    let source = "SELECT Code FROM Catalog.OpenSdblMetadataProbe";
    assert_eq!(
        QueryCompiler::new(&selected, PostgresBackend)
            .compile(source)
            .unwrap()
            .sql,
        QueryCompiler::new(&full, PostgresBackend)
            .compile(source)
            .unwrap()
            .sql
    );
    let backend = MsSqlBackend::new(2000).unwrap();
    assert_eq!(
        QueryCompiler::new(&selected, backend)
            .compile(source)
            .unwrap()
            .sql,
        QueryCompiler::new(&full, backend)
            .compile(source)
            .unwrap()
            .sql
    );
}

#[test]
fn selected_separators_keep_their_flags_and_report_missing_settings() {
    let full = support::separators_snapshot();
    let owner = full.object_id(MetadataKind::Catalog, "Товары").unwrap();
    let supplied = full
        .descriptors()
        .iter()
        .filter(|descriptor| descriptor.resource_guid.to_bytes() == owner.as_bytes())
        .cloned()
        .collect();
    let incomplete = resolve_selected_metadata(
        full.db_names(),
        supplied,
        vec![],
        vec![],
        full.schema(),
        full.live_tables(),
    );
    assert_eq!(
        incomplete.snapshot.separators().count(),
        full.separators()
            .filter(|field| field
                .owner_tables
                .iter()
                .any(|table| incomplete.snapshot.live_table(table).is_some()))
            .count()
    );
    assert!(
        incomplete
            .report
            .findings()
            .iter()
            .any(|finding| matches!(finding, ResolutionFinding::SeparatorSettingsMissing { .. }))
    );
    let supplied = full
        .descriptors()
        .iter()
        .filter(|descriptor| {
            descriptor.resource_guid.to_bytes() == owner.as_bytes()
                || full
                    .separators()
                    .any(|field| descriptor.resource_guid == field.guid)
                || descriptor.name == "ЗначениеРазделителя"
                || descriptor.name == "ИспользованиеРазделителя"
        })
        .cloned()
        .collect();
    let complete = resolve_selected_metadata(
        full.db_names(),
        supplied,
        vec![],
        vec![],
        full.schema(),
        full.live_tables(),
    )
    .snapshot;
    let mut session = SessionParameters::new();
    session.set(QueryParameter::new(
        "ЗначениеРазделителя",
        ParameterValue::Number {
            unscaled: 7,
            scale: 0,
        },
    ));
    session.set(QueryParameter::new(
        "ИспользованиеРазделителя",
        ParameterValue::Boolean(true),
    ));
    support::supply_active_separation_conditions(&full, &mut session);
    let options = CompileOptions::new().session(&session);
    let source = "SELECT Code FROM Catalog.Товары";
    assert_eq!(
        QueryCompiler::new(&complete, PostgresBackend)
            .compile_with(source, &options)
            .unwrap()
            .sql,
        QueryCompiler::new(&full, PostgresBackend)
            .compile_with(source, &options)
            .unwrap()
            .sql
    );
    // Публичный программный конструктор не используется для проекции разделителей.
    let rebuilt = DbNames::from_entries(full.db_names().entries().to_vec());
    assert!(
        full.separators()
            .any(|field| !rebuilt.is_data_separator(field.number))
    );
}

#[test]
fn selected_explicit_extension_keeps_its_field_and_table_mapping() {
    let full = support::snapshot();
    let owner = &full.objects()[0].guid;
    let field = support::guid("33333333-3333-4333-8333-333333333333");
    let mut extension_schema = full.schema().clone();
    let mut column = extension_schema.tables[0].columns.last().unwrap().clone();
    column.name = "Fld999".into();
    extension_schema.tables[0].columns.push(column);
    let extension = open_sdbl::metadata::ExtensionMetadata {
        origin: "explicit".into(),
        db_names: DbNames::from_entries(vec![open_sdbl::metadata::DbNameEntry {
            guid: field.clone(),
            alias: "Fld".into(),
            number: 999,
        }]),
        descriptors: vec![support::descriptor(owner, &field, "ExtensionField")],
        schema: extension_schema,
        field_reference_targets: vec![],
        restructure_anomalies: vec![],
    };
    let selected = resolve_selected_metadata(
        full.db_names(),
        full.descriptors().to_vec(),
        vec![],
        vec![extension],
        full.schema(),
        full.live_tables(),
    )
    .snapshot;
    let projected = selected
        .fields()
        .iter()
        .find(|entry| entry.guid == field)
        .unwrap();
    assert_eq!(projected.extension_origin.as_deref(), Some("explicit"));
    assert!(
        projected
            .owner_tables
            .iter()
            .any(|table| table == "_Reference53")
    );
}
