mod support;
use open_sdbl::metadata::{FieldId, StandardFieldId, resolve_metadata};
use open_sdbl::query::{PostgresBackend, QueryCompiler, object_query_name, queryable_fields};

#[test]
fn wildcard_and_queryable_fields_use_the_russian_reference_name() {
    let snapshot = support::populated_catalog_snapshot(1, 1);
    let object = &snapshot.objects()[0];
    let fields = queryable_fields(&snapshot, object).unwrap();
    let reference = fields
        .iter()
        .find(|f| f.field == Some(FieldId::Standard(StandardFieldId::Id)))
        .unwrap();
    assert_eq!(reference.name, "Ссылка");
    assert_eq!(reference.schema_name, "ID");
    assert!(reference.aliases.iter().any(|a| a == "ID"));
    let source = object_query_name(&snapshot, object).unwrap();
    let compiler = QueryCompiler::new(&snapshot, PostgresBackend);
    let star = compiler.compile(&format!("ВЫБРАТЬ * ИЗ {source}")).unwrap();
    assert_eq!(star.columns[0].label, "Ссылка");
    assert!(star.sql.contains("\"_idrref\""));
    let alias = compiler
        .compile(&format!("ВЫБРАТЬ Ссылка КАК ID ИЗ {source}"))
        .unwrap();
    assert_eq!(alias.columns[0].label, "ID");
    assert_eq!(
        alias.columns[0].origin.as_ref().unwrap().field,
        FieldId::Standard(StandardFieldId::Id)
    );
    assert!(compiler.compile(&format!("ВЫБРАТЬ ID ИЗ {source}")).is_ok());
}

#[test]
fn custom_id_name_is_not_replaced_by_the_standard_reference_name() {
    let base = support::populated_catalog_snapshot(1, 1);
    let mut descriptors = base.descriptors().to_vec();
    for d in &mut descriptors {
        if d.object_guid != d.resource_guid {
            d.name = "ID".into();
        }
    }
    let snapshot = resolve_metadata(
        base.db_names().clone(),
        descriptors,
        base.schema().clone(),
        base.live_tables().to_vec(),
    )
    .snapshot;
    let fields = queryable_fields(&snapshot, &snapshot.objects()[0]).unwrap();
    assert!(
        fields
            .iter()
            .any(|f| f.name == "ID" && matches!(f.field, Some(FieldId::Metadata(_))))
    );
    assert!(
        fields
            .iter()
            .any(|f| f.name == "Ссылка" && f.field == Some(FieldId::Standard(StandardFieldId::Id)))
    );
}

#[path = "support/section_reference.rs"]
mod section_reference;

#[test]
fn tabular_section_wildcard_uses_the_owner_reference_name() {
    let snapshot = section_reference::snapshot();
    let compiler = QueryCompiler::new(&snapshot, PostgresBackend);
    let compiled = compiler
        .compile("ВЫБРАТЬ * ИЗ Документ.ПриходнаяНакладная.Запасы")
        .unwrap();
    let reference = compiled
        .columns
        .iter()
        .find(|c| {
            c.origin
                .as_ref()
                .is_some_and(|o| o.field == FieldId::Standard(StandardFieldId::Id))
        })
        .unwrap();
    assert_eq!(reference.label, "Ссылка");
    assert_eq!(
        reference.origin.as_ref().unwrap().table_part.as_deref(),
        Some("Запасы")
    );
    let alias = compiler
        .compile("ВЫБРАТЬ Ссылка КАК ID ИЗ Документ.ПриходнаяНакладная.Запасы")
        .unwrap();
    assert_eq!(alias.columns[0].label, "ID");
}
