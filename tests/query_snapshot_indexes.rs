//! Snapshot lookup reuse must preserve identities, duplicate evidence and refresh.
mod support;
use open_sdbl::metadata::{AttributeId, ObjectId, resolve_metadata};
use open_sdbl::query::{PostgresBackend, QueryCompiler, queryable_field_catalog, queryable_fields};

#[test]
fn populated_snapshot_reuses_names_origins_and_types_across_preparations() {
    let snapshot = support::populated_catalog_snapshot(64, 16);
    assert_eq!(snapshot.fields().len(), 1024);
    let all = queryable_field_catalog(&snapshot);
    let compiler = QueryCompiler::new(&snapshot, PostgresBackend);
    for object in snapshot.objects() {
        let id = ObjectId::from(&object.guid);
        let fields = queryable_fields(&snapshot, object).unwrap();
        assert_eq!(all[&id], fields);
        let text = format!(
            "ВЫБРАТЬ Реквизит1,Реквизит16 ИЗ Справочник.{}",
            object.name.as_ref().unwrap()
        );
        let prepared = compiler.prepare(&text).unwrap();
        let first = prepared.compile(&snapshot, &[]).unwrap();
        let second = prepared.compile(&snapshot.clone(), &[]).unwrap();
        assert_eq!(first.columns, second.columns);
        assert!(first.columns.iter().all(|c| c.origin.is_some()));
    }
}
#[test]
fn descriptor_indexes_preserve_source_order_duplicates_and_snapshot_isolation() {
    let snapshot = support::populated_catalog_snapshot(2, 2);
    let field = &snapshot.fields()[0];
    let id = AttributeId::from(&field.guid);
    let mut descriptors = snapshot.descriptors().to_vec();
    let mut duplicate = snapshot.attribute_descriptors(id).next().unwrap().clone();
    duplicate.name = "НовоеИмя".into();
    duplicate.declared_types = None;
    descriptors.push(duplicate);
    let changed = resolve_metadata(
        snapshot.db_names().clone(),
        descriptors,
        snapshot.schema().clone(),
        snapshot.live_tables().to_vec(),
    )
    .snapshot;
    let names = changed
        .attribute_descriptors(id)
        .map(|d| d.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["Реквизит1", "НовоеИмя"]);
    assert_eq!(snapshot.attribute_descriptors(id).count(), 1);
    assert_eq!(
        changed
            .owner_descriptor(ObjectId::from(&snapshot.objects()[0].guid))
            .unwrap()
            .name,
        "Каталог1"
    );
    let mut descriptors = snapshot.descriptors().to_vec();
    for d in &mut descriptors {
        if d.object_guid == field.guid {
            d.name = "Переименован".into();
        }
    }
    let renamed = resolve_metadata(
        snapshot.db_names().clone(),
        descriptors,
        snapshot.schema().clone(),
        snapshot.live_tables().to_vec(),
    )
    .snapshot;
    let text = "ВЫБРАТЬ Реквизит1 ИЗ Справочник.Каталог1";
    assert!(
        QueryCompiler::new(&snapshot, PostgresBackend)
            .compile(text)
            .is_ok()
    );
    assert!(
        QueryCompiler::new(&renamed, PostgresBackend)
            .compile(text)
            .is_err()
    );
    assert!(
        QueryCompiler::new(&renamed, PostgresBackend)
            .compile("ВЫБРАТЬ Переименован ИЗ Справочник.Каталог1")
            .is_ok()
    );
}
