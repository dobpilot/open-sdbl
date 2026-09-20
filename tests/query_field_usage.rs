//! Where a query reads each field.
//!
//! Masking the output alone hides nothing: a statement that filters on an
//! attribute learns its value from which rows come back. These tests pin
//! that the report distinguishes the roles an application must refuse
//! from the one it can merely mask.

mod support;

use support::*;

use open_sdbl::metadata::{FieldId, MetadataSnapshot, ObjectId, StandardFieldId};
use open_sdbl::query::{
    FieldUsage, MsSqlBackend, PostgresBackend, QueryCompiler, find_metadata_object,
};

fn object_id(snapshot: &MetadataSnapshot, name: &str) -> ObjectId {
    ObjectId::from(&find_metadata_object(snapshot, name).unwrap().guid)
}

fn mssql() -> MsSqlBackend {
    MsSqlBackend::new(0).unwrap()
}

/// The report of a prepared query, checked to agree across dialects: it
/// describes the metadata a query reads, which no dialect changes.
fn usage(
    snapshot: &MetadataSnapshot,
    source: &str,
) -> Vec<(ObjectId, Option<String>, FieldId, FieldUsage)> {
    let flatten = |query: &open_sdbl::query::FieldUsageRequest| {
        query
            .fields
            .iter()
            .map(|used| (used.object, used.table_part.clone(), used.field, used.usage))
            .collect::<Vec<_>>()
    };
    let postgres = QueryCompiler::new(snapshot, PostgresBackend)
        .prepare(source)
        .unwrap_or_else(|error| panic!("{source}: {error}"));
    let sql_server = QueryCompiler::new(snapshot, mssql())
        .prepare(source)
        .unwrap_or_else(|error| panic!("{source}: {error}"));
    let left = flatten(postgres.field_usage());
    assert_eq!(left, flatten(sql_server.field_usage()), "{source}");
    left
}

fn roles_of(
    used: &[(ObjectId, Option<String>, FieldId, FieldUsage)],
    field: FieldId,
) -> Vec<FieldUsage> {
    let mut roles = used
        .iter()
        .filter(|entry| entry.2 == field)
        .map(|entry| entry.3)
        .collect::<Vec<_>>();
    roles.sort_unstable();
    roles
}

#[test]
fn a_field_read_only_by_the_filter_is_reported_as_a_predicate() {
    let snapshot = snapshot();
    let used = usage(
        &snapshot,
        "ВЫБРАТЬ Code ИЗ Справочник.OpenSdblMetadataProbe ГДЕ ProbeAttribute = 1",
    );
    let attribute = used
        .iter()
        .find(|entry| matches!(entry.2, FieldId::Metadata(_)))
        .expect("the attribute is reported");
    assert_eq!(attribute.3, FieldUsage::Filter);
    assert_eq!(
        roles_of(&used, FieldId::Standard(StandardFieldId::Code)),
        [FieldUsage::Projection],
        "the projected field is not reported as a predicate"
    );
}

#[test]
fn a_field_in_two_roles_is_reported_in_both() {
    let snapshot = snapshot();
    let used = usage(
        &snapshot,
        "ВЫБРАТЬ Code ИЗ Справочник.OpenSdblMetadataProbe ГДЕ Code = \"A\"",
    );
    let mut roles = roles_of(&used, FieldId::Standard(StandardFieldId::Code));
    roles.dedup();
    assert_eq!(
        roles,
        [FieldUsage::Projection, FieldUsage::Filter]
            .map(|r| r)
            .to_vec()
            .tap_sorted()
    );
}

trait TapSorted {
    fn tap_sorted(self) -> Self;
}

impl TapSorted for Vec<FieldUsage> {
    fn tap_sorted(mut self) -> Self {
        self.sort_unstable();
        self
    }
}

#[test]
fn an_aggregate_argument_and_an_expression_are_different_roles() {
    let snapshot = snapshot();
    let used = usage(
        &snapshot,
        "ВЫБРАТЬ КОЛИЧЕСТВО(Code) КАК К, ProbeAttribute
         ИЗ Справочник.OpenSdblMetadataProbe
         СГРУППИРОВАТЬ ПО ProbeAttribute",
    );
    assert_eq!(
        roles_of(&used, FieldId::Standard(StandardFieldId::Code)),
        [FieldUsage::Aggregate],
        "a field inside an aggregate is its argument, not a projection"
    );

    let expression = usage(
        &snapshot,
        "ВЫБРАТЬ Code + \"x\" КАК В ИЗ Справочник.OpenSdblMetadataProbe",
    );
    assert_eq!(
        roles_of(&expression, FieldId::Standard(StandardFieldId::Code)),
        [FieldUsage::Expression],
        "a field inside a computed expression is not projected on its own"
    );
}

#[test]
fn grouping_and_ordering_are_their_own_roles() {
    let snapshot = snapshot();
    let used = usage(
        &snapshot,
        "ВЫБРАТЬ ProbeAttribute ИЗ Справочник.OpenSdblMetadataProbe
         СГРУППИРОВАТЬ ПО ProbeAttribute",
    );
    assert!(used.iter().any(|entry| entry.3 == FieldUsage::Grouping));

    let ordered = usage(
        &snapshot,
        "ВЫБРАТЬ Code + \"x\" КАК В ИЗ Справочник.OpenSdblMetadataProbe УПОРЯДОЧИТЬ ПО Code",
    );
    assert!(
        roles_of(&ordered, FieldId::Standard(StandardFieldId::Code))
            .contains(&FieldUsage::Ordering)
    );
}

#[test]
fn a_join_condition_is_its_own_role() {
    let snapshot = snapshot();
    let used = usage(
        &snapshot,
        "ВЫБРАТЬ Л.Code ИЗ Справочник.OpenSdblMetadataProbe КАК Л
             ЛЕВОЕ СОЕДИНЕНИЕ Справочник.OpenSdblMetadataProbe КАК П ПО Л.Ссылка = П.Ссылка",
    );
    assert_eq!(
        roles_of(&used, FieldId::Standard(StandardFieldId::Id)),
        [FieldUsage::JoinCondition],
    );
}

#[test]
fn a_having_predicate_is_its_own_role() {
    let snapshot = snapshot();
    let used = usage(
        &snapshot,
        "ВЫБРАТЬ ProbeAttribute ИЗ Справочник.OpenSdblMetadataProbe
         СГРУППИРОВАТЬ ПО ProbeAttribute
         ИМЕЮЩИЕ КОЛИЧЕСТВО(Code) > 1",
    );
    assert_eq!(
        roles_of(&used, FieldId::Standard(StandardFieldId::Code)),
        [FieldUsage::Aggregate],
        "the field inside the aggregate is its argument; the clause is the aggregate's home"
    );
}

#[test]
fn a_dereference_names_the_object_the_path_ended_on() {
    let snapshot = tabular_section_snapshot();
    let centre = object_id(&snapshot, "Справочник.ЦентрыФинансовойОтветственности");
    let document = object_id(&snapshot, "Документ.бит_ДополнительныеУсловияПоДоговору");
    let used = usage(
        &snapshot,
        "ВЫБРАТЬ Т.ЦФО.Сам_БизнесРегион
         ИЗ Документ.бит_ДополнительныеУсловияПоДоговору.ГрафикНачислений КАК Т",
    );
    let target = used
        .iter()
        .find(|entry| entry.0 == centre)
        .expect("the counterparty catalog is reported");
    assert!(
        target.1.is_none(),
        "the dereference target is a whole object"
    );
    // The hop through the section's own field is reported against the
    // section, which is where it is read.
    assert!(
        used.iter()
            .any(|entry| entry.0 == document && entry.1.as_deref() == Some("ГрафикНачислений"))
    );
}

#[test]
fn a_tabular_section_is_named_with_its_section() {
    let snapshot = tabular_section_snapshot();
    let document = object_id(&snapshot, "Документ.бит_ДополнительныеУсловияПоДоговору");
    let used = usage(
        &snapshot,
        "ВЫБРАТЬ Т.Сумма ИЗ Документ.бит_ДополнительныеУсловияПоДоговору.ГрафикНачислений КАК Т",
    );
    assert!(
        used.iter()
            .all(|entry| entry.0 == document && entry.1.as_deref() == Some("ГрафикНачислений")),
        "{used:?}"
    );
}

#[test]
fn collecting_the_report_changes_no_sql() {
    let snapshot = snapshot();
    let source = "ВЫБРАТЬ Code ИЗ Справочник.OpenSdblMetadataProbe ГДЕ ProbeAttribute = 1";
    let compiled = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile(source)
        .unwrap();
    let prepared = QueryCompiler::new(&snapshot, PostgresBackend)
        .prepare(source)
        .unwrap()
        .compile(&snapshot, &[])
        .unwrap();
    assert_eq!(compiled.sql, prepared.sql);
    assert!(compiled.sql.starts_with("SELECT "), "{}", compiled.sql);
}
