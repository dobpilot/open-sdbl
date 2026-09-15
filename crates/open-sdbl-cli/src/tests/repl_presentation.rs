//! Tests of the console `presentation` module.

use std::collections::HashMap;

use open_sdbl::metadata::{
    FieldId, LiveTable, MetadataKind, ObjectId, SchemaStorage, StandardFieldId, parse_db_names,
    resolve_metadata,
};
use open_sdbl::query::PresentationExpression;

use super::*;

#[test]
fn splits_binary_deferred_payloads() {
    let mut payload = vec![0, 0, 0, 0xea];
    payload.extend_from_slice(&[7; 16]);
    assert_eq!(
        split_deferred_payload(&payload).unwrap(),
        Some((0xea, [7; 16]))
    );
    assert_eq!(split_deferred_payload(&[0; 20]).unwrap(), None);
    assert!(split_deferred_payload(&[0; 16]).is_err());
    assert!(split_deferred_payload(&[]).is_err());
}

#[test]
fn unresolved_deferred_presentations_are_visible() {
    // A reference that no object answers keeps the marker, while a row
    // carrying no reference at all has no presentation: the empty
    // reference resolves to nothing and a null column stays null, the
    // way the platform prints them.
    let object = ObjectId::from_bytes([7; 16]);
    assert_eq!(
        resolved_presentation(&HashMap::new(), object, [9; 16]),
        UNRESOLVED_REFERENCE
    );
    assert_eq!(split_deferred_payload(&[0; 20]).unwrap(), None);
}

#[test]
fn catalog_default_presentation_is_description_space_code() {
    let description = FieldId::Standard(StandardFieldId::Description);
    let code = FieldId::Standard(StandardFieldId::Code);
    let (fields, expression) = default_presentation_template(
        Some(MetadataKind::Catalog),
        "Номенклатура",
        Some(description),
        Some(code),
        None,
        None,
        None,
    );
    assert_eq!(fields, [description, code]);
    assert_eq!(
        expression,
        PresentationExpression::Concat(vec![
            PresentationExpression::Field(description),
            PresentationExpression::Literal(" (".to_owned()),
            PresentationExpression::Field(code),
            PresentationExpression::Literal(")".to_owned()),
        ])
    );
}

#[test]
fn document_default_presentation_is_type_number_and_period() {
    let number = FieldId::Standard(StandardFieldId::Number);
    let date = FieldId::Standard(StandardFieldId::Date);
    let (fields, expression) = default_presentation_template(
        Some(MetadataKind::Document),
        "Реализация товаров",
        None,
        None,
        Some(number),
        Some(date),
        None,
    );
    assert_eq!(fields, [number, date]);
    assert_eq!(
        expression,
        PresentationExpression::Concat(vec![
            PresentationExpression::Literal("Реализация товаров".to_owned()),
            PresentationExpression::Literal(" ".to_owned()),
            PresentationExpression::Field(number),
            PresentationExpression::Literal(" от ".to_owned()),
            PresentationExpression::Field(date),
        ])
    );
}

#[test]
fn presentation_cache_uses_the_production_lookup_and_clears_on_refresh() {
    let serialized = b"{1,{b8bac76b-c91b-4d78-8a70-ffa39f8de694,\"Reference\",53}}";
    let length = u16::try_from(serialized.len()).unwrap();
    let mut compressed = vec![1];
    compressed.extend_from_slice(&length.to_le_bytes());
    compressed.extend_from_slice(&(!length).to_le_bytes());
    compressed.extend_from_slice(serialized);
    let snapshot = resolve_metadata(
        parse_db_names(&compressed).unwrap(),
        Vec::new(),
        SchemaStorage {
            tables: Vec::new(),
            anomalies: Vec::new(),
        },
        Vec::<LiveTable>::new(),
    )
    .snapshot;
    let mut cache = HashMap::new();
    let object = ObjectId::from_bytes([7; 16]);
    let first = presentation_plan(&mut cache, &snapshot, object);
    let second = presentation_plan(&mut cache, &snapshot, object);
    assert_eq!(first, second);
    assert_eq!(cache.len(), 1);
    cache.clear();
    let refreshed = presentation_plan(&mut cache, &snapshot, object);
    assert_eq!(refreshed, first);
    assert_eq!(cache.len(), 1);
}
