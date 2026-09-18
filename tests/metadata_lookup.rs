mod support;

use support::{
    ambiguous_field_snapshot, ambiguous_object_snapshot, catalog_value_snapshot, guid, snapshot,
};

use open_sdbl::metadata::{
    AttributeId, ConfigPredefinedValue, LookupError, MetadataKind, ObjectId, PredefinedSource,
    StandardFieldId, resolve_metadata_with_predefined_values,
};

#[test]
fn looks_up_attributes_and_predefined_values_by_stable_identity() {
    let snapshot = snapshot();
    let owner = snapshot
        .object_id(MetadataKind::Catalog, "OpenSdblMetadataProbe")
        .unwrap();
    let attribute = snapshot.attribute_id(owner, "ProbeAttribute").unwrap();
    let resolved = snapshot.attribute_by_id(attribute).unwrap();
    assert_eq!(resolved.name.as_deref(), Some("ProbeAttribute"));
    assert_eq!(AttributeId::from(&resolved.guid), attribute);

    let snapshot = catalog_value_snapshot();
    let owner = snapshot
        .object_id(MetadataKind::Catalog, "OpenSdblMetadataProbe")
        .unwrap();
    let value = snapshot.predefined_value(owner, "Утвержден").unwrap();
    assert_eq!(value.name, "Утвержден");
    assert_eq!(value.guid.as_str(), "2e22ad88-32b5-4456-a3da-e56fa2f94623");
}

#[test]
fn exercises_every_lookup_error_variant() {
    let base = snapshot();
    let owner = base
        .object_id(MetadataKind::Catalog, "OpenSdblMetadataProbe")
        .unwrap();
    let missing_object = base
        .object_id(MetadataKind::Catalog, "Missing")
        .unwrap_err();

    let ambiguous_objects = ambiguous_object_snapshot();
    let ambiguous_object = ambiguous_objects
        .object_id(MetadataKind::Catalog, "Duplicate")
        .unwrap_err();

    let missing_field = base
        .attribute_by_id(AttributeId::from_bytes([0xff; 16]))
        .unwrap_err();
    let ambiguous_fields = ambiguous_field_snapshot();
    let ambiguous_field = ambiguous_fields
        .attribute_by_id(AttributeId::from(&guid(
            "03bd775a-e0a1-4205-82ce-6068e73ad134",
        )))
        .unwrap_err();

    let values = catalog_value_snapshot();
    let value_owner = values
        .object_id(MetadataKind::Catalog, "OpenSdblMetadataProbe")
        .unwrap();
    let missing_value = values.predefined_value(value_owner, "Missing").unwrap_err();

    let owner_guid = base.objects()[0].guid.clone();
    let ambiguous_values = resolve_metadata_with_predefined_values(
        base.db_names().clone(),
        base.descriptors().to_vec(),
        vec![
            ConfigPredefinedValue {
                owner_guid: owner_guid.clone(),
                value_guid: guid("11111111-1111-4111-8111-111111111111"),
                name: "DuplicateValue".to_owned(),
                source: PredefinedSource::Catalog,
            },
            ConfigPredefinedValue {
                owner_guid,
                value_guid: guid("22222222-2222-4222-8222-222222222222"),
                name: "DuplicateValue".to_owned(),
                source: PredefinedSource::Catalog,
            },
        ],
        base.schema().clone(),
        base.live_tables().to_vec(),
    )
    .snapshot;
    let ambiguous_owner = ambiguous_values
        .object_id(MetadataKind::Catalog, "OpenSdblMetadataProbe")
        .unwrap();
    let ambiguous_value = ambiguous_values
        .predefined_value(ambiguous_owner, "DuplicateValue")
        .unwrap_err();

    let standard_field = base.attribute_id(owner, "Code").unwrap_err();
    let missing_owner = base
        .field_id(ObjectId::from_bytes([0xee; 16]), "Anything")
        .unwrap_err();

    assert_eq!(
        [
            missing_object,
            ambiguous_object,
            missing_field,
            ambiguous_field,
            missing_value,
            ambiguous_value,
            standard_field,
            missing_owner,
        ],
        [
            LookupError::ObjectNotFound,
            LookupError::AmbiguousObject,
            LookupError::FieldNotFound,
            LookupError::AmbiguousField,
            LookupError::ValueNotFound,
            LookupError::AmbiguousValue,
            LookupError::StandardFieldHasNoMetadataGuid(StandardFieldId::Code),
            LookupError::OwnerNotFound,
        ]
    );
}

#[test]
fn reads_the_reference_targets_a_type_description_names() {
    // SchemaStorage names the target of a reference only when the field
    // admits one table; with several it writes an empty target. The list
    // lives in the Config type description instead: `{"Pattern", {"#",
    // <reference type>}, …}`, and the third identifier of an object's class
    // list is the reference type other objects name. Both were measured on
    // 8.3.27 against a probe configuration.
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/demo");
    let pack = std::fs::read(root.join("config.pack")).unwrap();
    let mut descriptors = Vec::new();
    let mut offset = 0usize;
    while offset < pack.len() {
        let newline = offset
            + pack[offset..]
                .iter()
                .position(|byte| *byte == b'\n')
                .unwrap();
        let header = std::str::from_utf8(&pack[offset..newline]).unwrap();
        let (resource, length) = header.split_once('\t').unwrap();
        let length: usize = length.parse().unwrap();
        let start = newline + 1;
        if let Ok(parsed) =
            open_sdbl::metadata::parse_config_descriptors(resource, &pack[start..start + length])
        {
            descriptors.extend(parsed);
        }
        offset = start + length;
    }
    assert!(
        descriptors
            .iter()
            .filter(|descriptor| descriptor.object_reference_type.is_some())
            .count()
            > 1000,
        "every stored object carries its reference type"
    );
    let composite = descriptors
        .iter()
        .filter(|descriptor| descriptor.reference_types.len() > 1)
        .count();
    assert!(composite > 100, "composite attributes name their targets");
    // Every named reference type is a real identifier, never the empty one.
    assert!(
        descriptors
            .iter()
            .flat_map(|descriptor| descriptor.reference_types.iter())
            .all(|reference_type| reference_type.to_1c_bytes() != [0u8; 16]),
        "a named reference type is never the empty identifier"
    );
}
