//! Localized synonyms and comments on the resolved objects and fields.
//!
//! The decoder has always read them from `Config`; these tests pin that
//! they reach the snapshot an application actually holds, and that the
//! selection rule — language, trimming, fallback — lives in one place.

mod support;

use support::*;

use open_sdbl::metadata::{
    ConfigDescriptor, MetadataKind, MetadataSnapshot, Synonym, resolve_metadata,
};

/// The probe snapshot with synonyms and a comment attached to the catalog
/// and to its attribute, the way a configuration carries them.
fn annotated_snapshot() -> MetadataSnapshot {
    let base = snapshot();
    let object = base
        .objects()
        .iter()
        .find(|object| object.kind == Some(MetadataKind::Catalog) && object.name.is_some())
        .expect("the probe snapshot has a catalog")
        .guid
        .clone();
    let attribute = base
        .fields()
        .iter()
        .find(|field| field.name.as_deref() == Some("ProbeAttribute"))
        .expect("the probe snapshot has the attribute")
        .guid
        .clone();
    let descriptors = base
        .descriptors()
        .iter()
        .cloned()
        .map(|descriptor: ConfigDescriptor| annotate(descriptor, &object, &attribute))
        .collect::<Vec<_>>();
    resolve_metadata(
        base.db_names().clone(),
        descriptors,
        base.schema().clone(),
        base.live_tables().to_vec(),
    )
    .snapshot
}

fn annotate(
    mut descriptor: ConfigDescriptor,
    object: &open_sdbl::metadata::Guid,
    attribute: &open_sdbl::metadata::Guid,
) -> ConfigDescriptor {
    if descriptor.object_guid == *object {
        descriptor.synonyms = vec![
            Synonym {
                language: "ru".to_owned(),
                text: "Номенклатура".to_owned(),
            },
            Synonym {
                language: "en".to_owned(),
                text: "Goods".to_owned(),
            },
        ];
        descriptor.comment = Some("Справочник товаров".to_owned());
    }
    if descriptor.object_guid == *attribute {
        descriptor.synonyms = vec![
            Synonym {
                language: "ru".to_owned(),
                text: "  Корр. счет  ".to_owned(),
            },
            Synonym {
                language: "de".to_owned(),
                text: "   ".to_owned(),
            },
        ];
    }
    descriptor
}

fn catalog(snapshot: &MetadataSnapshot) -> &open_sdbl::metadata::MetadataObject {
    snapshot
        .objects()
        .iter()
        .find(|object| object.kind == Some(MetadataKind::Catalog) && object.name.is_some())
        .expect("the probe snapshot has a catalog")
}

fn attribute(snapshot: &MetadataSnapshot) -> &open_sdbl::metadata::MetadataField {
    snapshot
        .fields()
        .iter()
        .find(|field| field.name.as_deref() == Some("ProbeAttribute"))
        .expect("the probe snapshot has the attribute")
}

#[test]
fn the_synonyms_the_fixture_carries_reach_the_resolved_items() {
    // The probe `Config` resource really carries these, so this test
    // exercises the decoded path end to end rather than a synthesized one.
    let snapshot = snapshot();
    let object = catalog(&snapshot);
    assert_eq!(object.synonym("ru"), Some("Open sdbl metadata probe"));
    assert_eq!(object.presentation("ru"), Some("Open sdbl metadata probe"));
    assert_eq!(
        object.name.as_deref(),
        Some("OpenSdblMetadataProbe"),
        "a synonym presents the object; the query language still addresses the name"
    );

    let field = attribute(&snapshot);
    assert_eq!(field.synonym("ru"), Some("Probe attribute"));
    assert_eq!(field.presentation("ru"), Some("Probe attribute"));
    assert_eq!(field.name.as_deref(), Some("ProbeAttribute"));
}

#[test]
fn an_attribute_synonym_reaches_the_resolved_field() {
    let snapshot = annotated_snapshot();
    let field = attribute(&snapshot);
    assert_eq!(field.synonym("ru"), Some("Корр. счет"), "trimmed");
    assert_eq!(field.presentation("ru"), Some("Корр. счет"));
    assert_eq!(field.name.as_deref(), Some("ProbeAttribute"));
    assert_eq!(field.synonyms.len(), 2, "source order is preserved");
}

#[test]
fn an_object_synonym_reaches_the_resolved_object_and_the_snapshot() {
    let snapshot = annotated_snapshot();
    let object = catalog(&snapshot);
    assert_eq!(object.synonym("ru"), Some("Номенклатура"));
    assert_eq!(object.synonym("en"), Some("Goods"));
    assert_eq!(object.comment.as_deref(), Some("Справочник товаров"));

    // The same answers through the identifier, for a caller that holds one.
    let id = open_sdbl::metadata::ObjectId::from(&object.guid);
    assert_eq!(snapshot.object_synonym(id, "ru"), Some("Номенклатура"));
    assert_eq!(snapshot.object_presentation(id, "ru"), Some("Номенклатура"));
}

#[test]
fn the_language_is_matched_case_insensitively() {
    let snapshot = annotated_snapshot();
    assert_eq!(catalog(&snapshot).synonym("RU"), Some("Номенклатура"));
    assert_eq!(catalog(&snapshot).synonym("Ru"), Some("Номенклатура"));
}

#[test]
fn a_missing_language_falls_back_to_the_metadata_name() {
    let snapshot = annotated_snapshot();
    let field = attribute(&snapshot);
    assert_eq!(field.synonym("fr"), None);
    assert_eq!(field.presentation("fr"), field.name.as_deref());
}

#[test]
fn a_blank_synonym_counts_as_absent() {
    let snapshot = annotated_snapshot();
    let field = attribute(&snapshot);
    assert_eq!(field.synonym("de"), None, "whitespace is not a synonym");
    assert_eq!(field.presentation("de"), Some("ProbeAttribute"));
}

#[test]
fn an_item_without_a_descriptor_carries_nothing_and_presents_its_name() {
    // This fixture's descriptors carry no synonym at all, which is what a
    // configuration looks like before anyone fills the presentations in.
    let snapshot = tabular_section_snapshot();
    let bare = snapshot
        .objects()
        .iter()
        .find(|object| object.name.is_some())
        .expect("the fixture has a named object");
    assert!(bare.synonyms.is_empty(), "{:?}", bare.name);
    assert!(bare.comment.is_none());
    assert_eq!(bare.synonym("ru"), None);
    assert_eq!(
        bare.presentation("ru"),
        bare.name.as_deref(),
        "the presentation falls back to the name"
    );

    let unknown = open_sdbl::metadata::ObjectId::from_bytes([0x5a; 16]);
    assert_eq!(snapshot.object_synonym(unknown, "ru"), None);
    assert_eq!(snapshot.object_presentation(unknown, "ru"), None);
}
