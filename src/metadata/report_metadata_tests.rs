#[test]
fn native_report_composition_matches_designer() {
    let ds = super::super::parse_config_descriptors(
        "8bac013d-e4dc-43c2-980d-6190972b6678",
        include_bytes!("../../tests/fixtures/report-object/ЗамерОтчета.deflate"),
    )
    .unwrap();
    let owner = ds.iter().find(|d| d.name == "ЗамерОтчета").unwrap();
    assert_eq!(owner.report_attributes.as_ref().unwrap().len(), 2);
    assert_eq!(owner.report_tabular_sections.as_ref().unwrap().len(), 1);
    assert_eq!(
        owner.report_object_type.as_ref().unwrap().as_str(),
        "1ece1a7b-59c8-4f1a-9c70-5577fbae18f4"
    );
    assert_eq!(
        ds.iter()
            .find(|d| d.name == "СтрокиПробы")
            .unwrap()
            .tabular_section_attributes
            .as_ref()
            .unwrap()
            .len(),
        2
    );
}

const ID: &str = "8bac013d-e4dc-43c2-980d-6190972b6678";
const ATTRS: &str = "7e7123e0-29e2-11d6-a3c7-0050bae0a776";
const SECTIONS: &str = "b077d780-29e2-11d6-a3c7-0050bae0a776";
fn values(v: &mut super::Value) -> &mut Vec<super::Value> {
    match v {
        super::Value::List(v) => v,
        _ => panic!("native list expected"),
    }
}
fn rows<'a>(v: &'a mut super::Value, id: &str) -> &'a mut Vec<super::Value> {
    values(
        values(v)
            .iter_mut()
            .find(|v| super::list(v).is_some_and(|r| super::atom(&r[0]) == Some(id)))
            .unwrap(),
    )
}
fn mutated(
    edit: impl FnOnce(&mut super::Value, &mut Vec<super::ConfigDescriptor>),
) -> Vec<super::ConfigDescriptor> {
    let mut v = super::super::parse_serialized(include_bytes!(
        "../../tests/fixtures/report-object/ЗамерОтчета.config"
    ))
    .unwrap();
    let mut ds = super::super::parse_config_descriptors(
        ID,
        include_bytes!("../../tests/fixtures/report-object/ЗамерОтчета.deflate"),
    )
    .unwrap();
    for d in &mut ds {
        d.report_attributes = None;
        d.report_tabular_sections = None;
        d.report_object_type = None;
        d.tabular_section_attributes = None;
    }
    edit(&mut v, &mut ds);
    super::project(&v, &ID.parse().unwrap(), &mut ds);
    ds
}
fn owner(ds: &[super::ConfigDescriptor]) -> &super::ConfigDescriptor {
    ds.iter().find(|d| d.object_guid.as_str() == ID).unwrap()
}
#[test]
fn malformed_missing_duplicate_attributes_are_unknown() {
    for mode in 0..4 {
        let ds = mutated(|v, ds| match mode {
            0 => {
                values(&mut values(&mut rows(v, ATTRS)[2])[0])[0] =
                    super::Value::Atom("999".into());
            }
            1 => {
                rows(v, ATTRS).pop();
            }
            2 => ds.retain(|d| d.name != "ФлагПробы"),
            3 => {
                let d = ds.iter().find(|d| d.name == "ФлагПробы").unwrap().clone();
                ds.push(d);
            }
            _ => unreachable!(),
        });
        assert!(owner(&ds).report_attributes.is_none());
        assert_eq!(
            owner(&ds).report_tabular_sections.as_ref().unwrap().len(),
            1
        );
    }
}
#[test]
fn malformed_sections_and_columns_preserve_uncertainty() {
    for mode in 0..3 {
        let ds = mutated(|v, ds| match mode {
            0 => {
                values(&mut values(&mut rows(v, SECTIONS)[2])[0])[0] =
                    super::Value::Atom("999".into());
            }
            1 => {
                rows(v, SECTIONS).pop();
            }
            2 => ds.retain(|d| d.name != "СтрокиПробы"),
            _ => unreachable!(),
        });
        assert!(owner(&ds).report_tabular_sections.is_none());
    }
    let ds = mutated(|_, ds| ds.retain(|d| d.name != "ТекстСтроки"));
    assert!(
        ds.iter()
            .find(|d| d.name == "СтрокиПробы")
            .unwrap()
            .tabular_section_attributes
            .is_none()
    );
    assert_eq!(
        owner(&ds).report_tabular_sections.as_ref().unwrap().len(),
        1
    );
}
#[test]
fn unsupported_report_owner_header_is_unknown() {
    let ds = mutated(|v, _| {
        values(&mut values(&mut values(v)[1])[3])[0] = super::Value::Atom("999".into())
    });
    assert!(owner(&ds).report_attributes.is_none());
    assert!(owner(&ds).report_tabular_sections.is_none());
    assert!(owner(&ds).report_object_type.is_none());
}
#[test]
fn verified_empty_collections_are_distinct_from_unknown() {
    let ds = mutated(|v, _| {
        for id in [ATTRS, SECTIONS] {
            let r = rows(v, id);
            r.truncate(2);
            r[1] = super::Value::Atom("0".into());
        }
    });
    assert_eq!(owner(&ds).report_attributes, Some(vec![]));
    assert_eq!(owner(&ds).report_tabular_sections, Some(vec![]));
}

#[test]
fn actual_native_empty_report_and_original_unf_report_are_complete() {
    for (id, bytes, object) in [
        (
            ID,
            include_bytes!("../../tests/fixtures/report-object/empty.deflate").as_slice(),
            "1ece1a7b-59c8-4f1a-9c70-5577fbae18f4",
        ),
        (
            "9f7ccd25-3592-4dad-80e4-a47a0d144cce",
            include_bytes!("../../tests/fixtures/report-object/unf.deflate").as_slice(),
            "ff04dc15-ee00-446f-b3b7-5f8b5ac65306",
        ),
    ] {
        let ds = super::super::parse_config_descriptors(id, bytes).unwrap();
        let d = ds.iter().find(|d| d.object_guid.as_str() == id).unwrap();
        assert_eq!(d.report_attributes, Some(vec![]));
        assert_eq!(d.report_tabular_sections, Some(vec![]));
        assert_eq!(d.report_object_type.as_ref().unwrap().as_str(), object);
    }
}
#[test]
fn ordered_guids_and_qualified_types_match_independent_designer_xml() {
    let ds = mutated(|_, _| {});
    let oracle = include_str!("../../tests/fixtures/report-object/members.tsv")
        .lines()
        .map(|l| l.split('\t').collect::<Vec<_>>())
        .collect::<Vec<_>>();
    for (kind, ids) in [
        ("attribute", &owner(&ds).report_attributes),
        ("section", &owner(&ds).report_tabular_sections),
    ] {
        assert_eq!(
            ids.as_ref()
                .unwrap()
                .iter()
                .map(|id| id.as_str())
                .collect::<Vec<_>>(),
            oracle
                .iter()
                .filter(|r| r[0] == kind)
                .map(|r| r[2])
                .collect::<Vec<_>>()
        );
    }
    let section = ds.iter().find(|d| d.name == "СтрокиПробы").unwrap();
    assert_eq!(
        section
            .tabular_section_attributes
            .as_ref()
            .unwrap()
            .iter()
            .map(|id| id.as_str())
            .collect::<Vec<_>>(),
        oracle
            .iter()
            .filter(|r| r[0] == "field")
            .map(|r| r[2])
            .collect::<Vec<_>>()
    );
    for row in oracle
        .iter()
        .filter(|r| matches!(r[0], "attribute" | "field"))
    {
        let d = ds
            .iter()
            .find(|d| d.object_guid.as_str() == row[2])
            .unwrap();
        assert_eq!(d.name, row[3]);
        let types = d.declared_types.as_ref().unwrap();
        assert_eq!(types.len(), 1);
        if d.name.contains("Текст") {
            assert_eq!(types[0].tag, "S");
            assert_eq!(
                types[0].qualifiers,
                vec![
                    super::Value::Atom("12".into()),
                    super::Value::Atom("1".into())
                ]
            );
        } else {
            assert_eq!(types[0].tag, "B");
        }
    }
}

#[test]
fn unresolved_named_types_invalidate_complete_attributes_and_section_columns() {
    let mut ds = mutated(|_, _| {});
    let id: super::Guid = "95691e8c-6f02-43b4-9bcc-22d6111f877f".parse().unwrap();
    for d in &mut ds {
        if ["ТекстПробы", "ТекстСтроки"].contains(&d.name.as_str()) {
            d.declared_types = Some(vec![super::ConfigDeclaredType {
                tag: "#".into(),
                reference: Some(id.clone()),
                qualifiers: vec![],
            }]);
        }
    }
    let mut definition = owner(&ds).clone();
    definition.object_guid = id.clone();
    definition.defined_type_id = Some(id);
    definition.declared_types = None;
    ds.push(definition);
    super::resolve_named_types(&mut ds);
    assert!(owner(&ds).report_attributes.is_none());
    assert!(
        ds.iter()
            .find(|d| d.name == "СтрокиПробы")
            .unwrap()
            .tabular_section_attributes
            .is_none()
    );
    assert_eq!(
        owner(&ds).report_tabular_sections.as_ref().unwrap().len(),
        1
    );
}
