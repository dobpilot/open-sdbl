use open_sdbl::metadata::{
    ConfigDeclaredType, DbNames, MetadataKind, ReferenceTypeSet, SchemaStorage, Value,
    parse_config_resource_bounded, resolve_selected_metadata, resolve_selected_runtime_metadata,
};

#[allow(clippy::duplicate_mod)]
#[path = "support/mod.rs"]
mod support;

fn reference(id: &str) -> ConfigDeclaredType {
    ConfigDeclaredType {
        tag: "#".into(),
        reference: Some(support::guid(id)),
        qualifiers: vec![],
    }
}

#[test]
fn native_config_generic_families_agree_with_paired_xml_and_edt() {
    let bytes = include_bytes!("fixtures/generic-reference-0941/config.deflate");
    let parsed = parse_config_resource_bounded(
        "4062674a-4a81-494d-82bc-4984cd7c7903",
        bytes,
        64 * 1024 * 1024,
    )
    .unwrap();
    let field = parsed
        .descriptors
        .iter()
        .find(|d| d.object_guid.as_str() == "619b534d-5605-40d3-9b8c-4443a21d7795")
        .unwrap();
    let types = field.declared_types.as_ref().unwrap();
    let before = types.clone();
    let expected = [
        (MetadataKind::ExchangePlan, "ExchangePlanRef"),
        (MetadataKind::BusinessProcess, "BusinessProcessRef"),
        (MetadataKind::Document, "DocumentRef"),
        (
            MetadataKind::ChartOfCalculationTypes,
            "ChartOfCalculationTypesRef",
        ),
        (MetadataKind::Task, "TaskRef"),
        (
            MetadataKind::ChartOfCharacteristicTypes,
            "ChartOfCharacteristicTypesRef",
        ),
        (MetadataKind::ChartOfAccounts, "ChartOfAccountsRef"),
        (MetadataKind::Catalog, "CatalogRef"),
    ];
    assert_eq!(types.len(), expected.len());
    let xml = include_str!("fixtures/generic-reference-0941/Catalog.xml");
    let edt = include_str!("fixtures/generic-reference-0941/Catalog.mdo");
    let xml_field = xml
        .split_once("<Attribute uuid=\"619b534d-5605-40d3-9b8c-4443a21d7795\">")
        .unwrap()
        .1
        .split_once("</Attribute>")
        .unwrap()
        .0;
    let edt_field = edt
        .split_once("<attributes uuid=\"619b534d-5605-40d3-9b8c-4443a21d7795\">")
        .unwrap()
        .1
        .split_once("</attributes>")
        .unwrap()
        .0;
    let xml_types: Vec<_> = xml_field
        .split("<v8:TypeSet>cfg:")
        .skip(1)
        .map(|s| s.split_once("</v8:TypeSet>").unwrap().0)
        .collect();
    let edt_types: Vec<_> = edt_field
        .split("<types>")
        .skip(1)
        .map(|s| s.split_once("</types>").unwrap().0)
        .collect();
    let expected_names: Vec<_> = expected.iter().map(|(_, name)| *name).collect();
    assert_eq!(xml_types, expected_names);
    assert_eq!(edt_types, expected_names);
    for (ty, (kind, native_name)) in types.iter().zip(expected) {
        assert_eq!(ty.reference_type_set(), Some(ReferenceTypeSet::Kind(kind)));
        assert!(ty.qualifiers.is_empty());
        assert!(xml.contains(&format!("<v8:TypeSet>cfg:{native_name}</v8:TypeSet>")));
        assert!(edt.contains(&format!("<types>{native_name}</types>")));
    }
    assert_eq!(types, &before);
}

#[test]
fn any_membership_is_the_existing_nine_reference_kinds_and_overlap_is_deduplicable() {
    let any = reference("280f5f0e-9c8a-49cc-bf6d-4d296cc17a63")
        .reference_type_set()
        .unwrap();
    let enumeration = reference("474c3bf6-08b5-4ddc-a2ad-989cedf11583")
        .reference_type_set()
        .unwrap();
    assert_eq!(any, ReferenceTypeSet::Any);
    let kinds = [
        MetadataKind::Catalog,
        MetadataKind::Document,
        MetadataKind::Enumeration,
        MetadataKind::ChartOfCharacteristicTypes,
        MetadataKind::ChartOfCalculationTypes,
        MetadataKind::ChartOfAccounts,
        MetadataKind::ExchangePlan,
        MetadataKind::BusinessProcess,
        MetadataKind::Task,
    ];
    let mut members = std::collections::HashSet::new();
    for family in [
        any,
        enumeration,
        ReferenceTypeSet::Kind(MetadataKind::Catalog),
        any,
    ] {
        members.extend(kinds.into_iter().filter(|kind| family.contains_kind(*kind)));
    }
    assert_eq!(members.len(), 9);
    for kind in kinds {
        assert!(any.contains_kind(kind));
    }
    for kind in [
        MetadataKind::InformationRegister,
        MetadataKind::AccumulationRegister,
        MetadataKind::DocumentJournal,
    ] {
        assert!(!any.contains_kind(kind));
    }
}

#[test]
fn primitive_and_unknown_references_remain_unknown_without_rewriting_qualifiers() {
    let mut primitive = reference("e61ef7b8-f3e1-4f4b-8ac7-676e90524997");
    primitive.tag = "N".into();
    primitive.qualifiers = vec![Value::Atom("12".into()), Value::Atom("3".into())];
    let unknown = reference("11111111-1111-4111-8111-111111111111");
    let before = (primitive.clone(), unknown.clone());
    assert_eq!(primitive.reference_type_set(), None);
    assert_eq!(unknown.reference_type_set(), None);
    assert_eq!((primitive, unknown), before);
}

#[test]
fn aliases_keep_generic_families_qualifiers_and_unknown_members_in_all_metadata_views() {
    let root = support::guid("11111111-1111-4111-8111-111111111111");
    let attribute_id = support::guid("22222222-2222-4222-8222-222222222222");
    let alias = support::guid("33333333-3333-4333-8333-333333333333");
    let generic = reference("e61ef7b8-f3e1-4f4b-8ac7-676e90524997");
    let number = ConfigDeclaredType {
        tag: "N".into(),
        reference: None,
        qualifiers: vec![Value::Atom("12".into()), Value::Atom("3".into())],
    };
    let unknown = reference("44444444-4444-4444-8444-444444444444");
    let mut definition = support::descriptor(&root, &root, "Alias");
    definition.defined_type_id = Some(alias.clone());
    definition.declared_types = Some(vec![generic.clone(), number.clone(), unknown.clone()]);
    let mut attribute = support::descriptor(&root, &attribute_id, "Attribute");
    attribute.declared_types = Some(vec![reference(alias.as_str())]);
    let names = DbNames::from_entries(vec![]);
    let schema = SchemaStorage {
        tables: vec![],
        anomalies: vec![],
    };
    let descriptors = vec![definition, attribute];
    let runtime = resolve_selected_runtime_metadata(
        &names,
        descriptors.clone(),
        vec![],
        vec![],
        &schema,
        &[],
    );
    let full = resolve_selected_metadata(&names, descriptors, vec![], vec![], &schema, &[]);
    let finalized = runtime.finalize_query();
    let expected = vec![generic, number, unknown];
    for facts in [
        &runtime,
        full.snapshot.runtime_facts(),
        finalized.snapshot.runtime_facts(),
    ] {
        let declaration = facts
            .descriptors()
            .iter()
            .find(|d| d.object_guid == attribute_id)
            .unwrap()
            .declared_types
            .as_ref()
            .unwrap();
        assert_eq!(declaration, &expected);
        assert_eq!(
            declaration[0].reference_type_set(),
            Some(ReferenceTypeSet::Kind(MetadataKind::Catalog))
        );
        assert_eq!(declaration[2].reference_type_set(), None);
        assert_eq!(declaration[2].reference, expected[2].reference);
    }
    assert_eq!(
        full.snapshot.fingerprint(),
        finalized.snapshot.fingerprint()
    );
}

#[test]
fn public_kind_cannot_turn_a_nonreference_class_into_a_family() {
    for kind in [
        MetadataKind::Constant,
        MetadataKind::InformationRegister,
        MetadataKind::DocumentJournal,
    ] {
        assert!(!ReferenceTypeSet::Kind(kind).contains_kind(kind));
    }
    assert!(ReferenceTypeSet::Kind(MetadataKind::Catalog).contains_kind(MetadataKind::Catalog));
    assert!(!ReferenceTypeSet::Kind(MetadataKind::Catalog).contains_kind(MetadataKind::Document));
}

#[test]
fn lazy_reached_catalog_does_not_become_a_complete_generic_family() {
    let root = support::guid("11111111-1111-4111-8111-111111111111");
    let field = support::guid("22222222-2222-4222-8222-222222222222");
    let ref_id = support::guid("33333333-3333-4333-8333-333333333333");
    let mut owner = support::descriptor(&root, &root, "OneReachedCatalog");
    owner.object_reference_type = Some(ref_id);
    let generic = reference("e61ef7b8-f3e1-4f4b-8ac7-676e90524997");
    let mut attribute = support::descriptor(&root, &field, "Attribute");
    attribute.declared_types = Some(vec![generic.clone()]);
    let names = DbNames::from_entries(vec![open_sdbl::metadata::DbNameEntry {
        guid: root,
        alias: "Reference".into(),
        number: 1,
    }]);
    let schema = SchemaStorage {
        tables: vec![],
        anomalies: vec![],
    };
    let runtime = resolve_selected_runtime_metadata(
        &names,
        vec![owner, attribute],
        vec![],
        vec![],
        &schema,
        &[],
    );
    let ty = &runtime
        .descriptors()
        .iter()
        .find(|d| d.object_guid == field)
        .unwrap()
        .declared_types
        .as_ref()
        .unwrap()[0];
    assert_eq!(ty, &generic);
    assert_eq!(
        ty.reference_type_set(),
        Some(ReferenceTypeSet::Kind(MetadataKind::Catalog))
    );
}

#[test]
fn cyclic_alias_preserves_unknown_state_instead_of_a_partial_family() {
    let root = support::guid("11111111-1111-4111-8111-111111111111");
    let alias = support::guid("33333333-3333-4333-8333-333333333333");
    let mut definition = support::descriptor(&root, &root, "Cycle");
    definition.defined_type_id = Some(alias.clone());
    definition.declared_types = Some(vec![
        reference("e61ef7b8-f3e1-4f4b-8ac7-676e90524997"),
        reference(alias.as_str()),
    ]);
    let names = DbNames::from_entries(vec![]);
    let schema = SchemaStorage {
        tables: vec![],
        anomalies: vec![],
    };
    let runtime =
        resolve_selected_runtime_metadata(&names, vec![definition], vec![], vec![], &schema, &[]);
    assert!(runtime.descriptors()[0].declared_types.is_none());
    assert!(
        runtime.finalize_query().snapshot.descriptors()[0]
            .declared_types
            .is_none()
    );
}
