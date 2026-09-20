//! Where a result column came from.
//!
//! A consumer that must hide the value of an attribute cannot match on the
//! label: an alias replaces it, the provider truncates it, and duplicates
//! take suffixes. These tests pin that the origin survives all three and
//! that a column which is not a field says so.

mod support;

use support::*;

use open_sdbl::metadata::{FieldId, MetadataSnapshot, ObjectId, StandardFieldId};
use open_sdbl::query::{
    Backend, CompiledQuery, MsSqlBackend, PostgresBackend, QueryCompiler, find_metadata_object,
};

fn object_id(snapshot: &MetadataSnapshot, name: &str) -> ObjectId {
    ObjectId::from(&find_metadata_object(snapshot, name).unwrap().guid)
}

fn mssql() -> MsSqlBackend {
    MsSqlBackend::new(0).unwrap()
}

/// Compiles on both dialects and checks they agree about the origins,
/// which are a property of the metadata and not of the SQL text.
fn compile_both(snapshot: &MetadataSnapshot, source: &str) -> CompiledQuery {
    let postgres = QueryCompiler::new(snapshot, PostgresBackend)
        .compile(source)
        .unwrap_or_else(|error| panic!("{source}: {error}"));
    let mssql = QueryCompiler::new(snapshot, mssql())
        .compile(source)
        .unwrap_or_else(|error| panic!("{source}: {error}"));
    let origins = |query: &CompiledQuery| {
        query
            .columns
            .iter()
            .map(|column| column.origin.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(origins(&postgres), origins(&mssql), "{source}");
    postgres
}

/// One column's origin flattened for comparison across dialects.
type Origin = (ObjectId, Option<String>, FieldId, bool);

fn origins_of<B: Backend>(
    snapshot: &MetadataSnapshot,
    backend: B,
    source: &str,
) -> Vec<Option<Origin>> {
    QueryCompiler::new(snapshot, backend)
        .compile(source)
        .unwrap()
        .columns
        .iter()
        .map(|column| {
            column.origin.as_ref().map(|origin| {
                (
                    origin.object,
                    origin.table_part.clone(),
                    origin.field,
                    origin.composite_member,
                )
            })
        })
        .collect()
}

#[test]
fn an_alias_does_not_hide_where_a_column_came_from() {
    let snapshot = snapshot();
    let catalog = object_id(&snapshot, "Справочник.OpenSdblMetadataProbe");
    let aliased = compile_both(
        &snapshot,
        "ВЫБРАТЬ Т.Code КАК Х ИЗ Справочник.OpenSdblMetadataProbe КАК Т",
    );
    assert_eq!(aliased.columns[0].label, "Х");
    let origin = aliased.columns[0].origin.as_ref().expect("a field column");
    assert_eq!(origin.object, catalog);
    assert_eq!(origin.field, FieldId::Standard(StandardFieldId::Code));
    assert!(origin.table_part.is_none());
    assert!(!origin.composite_member);
}

#[test]
fn every_projected_field_carries_its_origin() {
    let snapshot = snapshot();
    let catalog = object_id(&snapshot, "Справочник.OpenSdblMetadataProbe");
    let compiled = compile_both(
        &snapshot,
        "ВЫБРАТЬ Ссылка, Code, ProbeAttribute ИЗ Справочник.OpenSdblMetadataProbe",
    );
    let fields = compiled
        .columns
        .iter()
        .map(|column| column.origin.as_ref().map(|origin| origin.field))
        .collect::<Vec<_>>();
    assert_eq!(fields[0], Some(FieldId::Standard(StandardFieldId::Id)));
    assert_eq!(fields[1], Some(FieldId::Standard(StandardFieldId::Code)));
    assert!(
        matches!(fields[2], Some(FieldId::Metadata(_))),
        "a Config attribute is named by its metadata identity: {fields:?}"
    );
    assert!(
        compiled
            .columns
            .iter()
            .all(|column| column.origin.as_ref().is_some_and(|o| o.object == catalog))
    );
}

#[test]
fn an_expression_an_aggregate_and_a_literal_have_no_origin() {
    let snapshot = snapshot();
    let compiled = compile_both(
        &snapshot,
        "ВЫБРАТЬ КОЛИЧЕСТВО(Code) КАК К, 1 КАК Л, ВЫБОР КОГДА ИСТИНА ТОГДА 1 ИНАЧЕ 2 КОНЕЦ КАК В
         ИЗ Справочник.OpenSdblMetadataProbe",
    );
    assert!(
        compiled
            .columns
            .iter()
            .all(|column| column.origin.is_none()),
        "a column that is not a field has no origin: {:?}",
        compiled.columns
    );
}

#[test]
fn a_tabular_section_names_its_owner_and_its_section() {
    let snapshot = tabular_section_snapshot();
    let document = object_id(&snapshot, "Документ.бит_ДополнительныеУсловияПоДоговору");
    let compiled = compile_both(
        &snapshot,
        "ВЫБРАТЬ Т.Сумма ИЗ Документ.бит_ДополнительныеУсловияПоДоговору.ГрафикНачислений КАК Т",
    );
    let origin = compiled.columns[0].origin.as_ref().unwrap();
    assert_eq!(origin.object, document);
    assert_eq!(origin.table_part.as_deref(), Some("ГрафикНачислений"));
}

#[test]
fn a_nested_section_result_carries_origins_too() {
    let snapshot = tabular_section_snapshot();
    let document = object_id(&snapshot, "Документ.бит_ДополнительныеУсловияПоДоговору");
    let compiled = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile("ВЫБРАТЬ Ссылка, ГрафикНачислений ИЗ Документ.бит_ДополнительныеУсловияПоДоговору")
        .unwrap();
    let section = compiled.nested.first().expect("one nested section");
    let named = section
        .columns
        .iter()
        .filter_map(|column| column.origin.as_ref())
        .collect::<Vec<_>>();
    assert!(!named.is_empty(), "the section columns carry origins");
    assert!(named.iter().all(|origin| origin.object == document
        && origin.table_part.as_deref() == Some("ГрафикНачислений")));
    // The owner key the consumer joins on is not a field of the section.
    assert!(
        section.columns[section.key_column].origin.is_none(),
        "the owner key is a service column"
    );
}

#[test]
fn every_member_of_a_composite_field_names_the_same_field() {
    let snapshot = mixed_composite_snapshot();
    let compiled = compile_both(
        &snapshot,
        "ВЫБРАТЬ Т.ProbeAttribute ИЗ Справочник.OpenSdblMetadataProbe КАК Т",
    );
    assert!(
        compiled.columns.len() > 1,
        "the fixture's attribute spreads over several columns"
    );
    let first = compiled.columns[0].origin.clone().expect("a field column");
    assert!(first.composite_member);
    for column in &compiled.columns {
        let origin = column.origin.as_ref().expect("every member is the field");
        assert_eq!(origin.field, first.field);
        assert!(origin.composite_member);
    }
}

#[test]
fn a_dereference_names_the_target_not_the_source() {
    let snapshot = tabular_section_snapshot();
    let centre = object_id(&snapshot, "Справочник.ЦентрыФинансовойОтветственности");
    let compiled = compile_both(
        &snapshot,
        "ВЫБРАТЬ Т.ЦФО.Сам_БизнесРегион
         ИЗ Документ.бит_ДополнительныеУсловияПоДоговору.ГрафикНачислений КАК Т",
    );
    let origin = compiled.columns[0].origin.as_ref().unwrap();
    assert_eq!(
        origin.object, centre,
        "the column reads the counterparty, not the document"
    );
    assert!(
        origin.table_part.is_none(),
        "the dereference target is a whole object, not the section the source reads"
    );
}

#[test]
fn a_suffixed_label_keeps_its_origin() {
    let snapshot = snapshot();
    // Two projections of one field would take one label, so the second is
    // suffixed; both still name the field.
    let compiled = compile_both(
        &snapshot,
        "ВЫБРАТЬ Л.Code, П.Code ИЗ Справочник.OpenSdblMetadataProbe КАК Л
             ЛЕВОЕ СОЕДИНЕНИЕ Справочник.OpenSdblMetadataProbe КАК П ПО Л.Ссылка = П.Ссылка",
    );
    assert_ne!(
        compiled.columns[0].label, compiled.columns[1].label,
        "the labels were made unique"
    );
    for column in &compiled.columns {
        assert_eq!(
            column.origin.as_ref().map(|origin| origin.field),
            Some(FieldId::Standard(StandardFieldId::Code))
        );
    }
}

#[test]
fn the_generated_sql_is_unchanged_by_the_origin() {
    // The origin is metadata about the column, computed from values the
    // compiler already held; it must not reach the statement.
    let snapshot = snapshot();
    let source = "ВЫБРАТЬ Т.Code КАК Х ИЗ Справочник.OpenSdblMetadataProbe КАК Т";
    let compiled = compile_both(&snapshot, source);
    assert_eq!(
        compiled.sql,
        "SELECT \"Т\".\"_code\"::text AS \"Х\" FROM \"_reference53\" AS \"Т\""
    );
    assert_eq!(
        origins_of(&snapshot, PostgresBackend, source),
        origins_of(&snapshot, mssql(), source),
        "the origin does not depend on the dialect"
    );
}
