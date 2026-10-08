use super::*;
const ID: &str = "ff92317d-71e7-4664-a13a-18bd31fd3701";
const TEXT: &[u8] = include_bytes!("../../tests/fixtures/data-processor-0911/Замеры.config");
fn values(v: &mut Value) -> &mut Vec<Value> {
    match v {
        Value::List(v) => v,
        _ => panic!("native list expected"),
    }
}
fn mutated(edit: impl FnOnce(&mut Value, &mut Vec<ConfigDescriptor>)) -> Vec<ConfigDescriptor> {
    let mut value = super::super::parse_serialized(TEXT).unwrap();
    let mut ds = super::super::parse_config_descriptors(
        ID,
        include_bytes!("../../tests/fixtures/data-processor-0911/Замеры.deflate"),
    )
    .unwrap();
    for d in &mut ds {
        d.data_processor_attributes = None;
        d.data_processor_tabular_sections = None;
        d.data_processor_object_type = None;
        d.tabular_section_attributes = None;
    }
    edit(&mut value, &mut ds);
    project(&value, &ID.parse().unwrap(), &mut ds);
    ds
}
fn owner(ds: &[ConfigDescriptor]) -> &ConfigDescriptor {
    ds.iter().find(|d| d.object_guid.as_str() == ID).unwrap()
}
#[test]
fn malformed_attribute_wrappers_and_missing_descriptors_invalidate_the_complete_list() {
    for mode in 0..4 {
        let ds = mutated(|v, ds| {
            let root = values(v);
            let c = root
                .iter_mut()
                .find(|v| {
                    list(v).is_some_and(|r| {
                        atom(&r[0]) == Some("ec6bb5e5-b7a8-4d75-bec9-658107a699cf")
                    })
                })
                .unwrap();
            let rows = values(c);
            match mode {
                0 => {
                    values(&mut values(&mut rows[2])[0])[0] = Value::Atom("999".into());
                }
                1 => {
                    values(&mut rows[2]).push(Value::Atom("999".into()));
                }
                2 => {
                    ds.retain(|d| d.name != "ФлагПробы");
                }
                3 => {
                    let d = ds.iter().find(|d| d.name == "ФлагПробы").unwrap().clone();
                    ds.push(d);
                }
                _ => unreachable!(),
            }
        });
        assert!(owner(&ds).data_processor_attributes.is_none());
        assert_eq!(
            owner(&ds)
                .data_processor_tabular_sections
                .as_ref()
                .unwrap()
                .len(),
            3
        );
    }
}
#[test]
fn malformed_section_wrappers_and_duplicate_members_are_not_complete() {
    for mode in 0..3 {
        let ds = mutated(|v, _| {
            let c = &mut values(v)[3];
            let rows = values(c);
            match mode {
                0 => values(&mut values(&mut rows[2])[0])[0] = Value::Atom("999".into()),
                1 => rows[3] = rows[2].clone(),
                2 => {
                    rows.pop();
                }
                _ => unreachable!(),
            }
        });
        assert!(owner(&ds).data_processor_tabular_sections.is_none());
        assert_eq!(
            owner(&ds).data_processor_attributes.as_ref().unwrap().len(),
            2
        );
    }
}
#[test]
fn section_field_corruption_does_not_mix_neighbouring_sections() {
    let ds = mutated(|v, _| {
        let section = &mut values(&mut values(v)[3])[2];
        values(&mut values(section)[2])[1] = Value::Atom("3".into());
    });
    assert!(
        ds.iter()
            .find(|d| d.name == "СтрокиПробы")
            .unwrap()
            .tabular_section_attributes
            .is_none()
    );
    assert_eq!(
        ds.iter()
            .find(|d| d.name == "ДругаяЧасть")
            .unwrap()
            .tabular_section_attributes
            .as_ref()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        ds.iter()
            .find(|d| d.name == "ПустаяЧасть")
            .unwrap()
            .tabular_section_attributes,
        Some(vec![])
    );
}
#[test]
fn unresolvable_types_invalidate_owner_and_section_field_completeness() {
    let mut ds = mutated(|_, _| {});
    let id: Guid = "95691e8c-6f02-43b4-9bcc-22d6111f877f".parse().unwrap();
    for d in &mut ds {
        if ["ТекстПробы", "ТекстСтроки"].contains(&d.name.as_str()) {
            d.declared_types = Some(vec![ConfigDeclaredType {
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
    resolve_named_types(&mut ds);
    assert!(owner(&ds).data_processor_attributes.is_none());
    assert!(
        ds.iter()
            .find(|d| d.name == "СтрокиПробы")
            .unwrap()
            .tabular_section_attributes
            .is_none()
    );
    assert_eq!(
        owner(&ds)
            .data_processor_tabular_sections
            .as_ref()
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn unmeasured_owner_header_wrapper_is_unknown() {
    let ds = mutated(|v, _| {
        values(&mut values(&mut values(v)[1])[3])[0] = Value::Atom("999".into());
    });
    assert!(owner(&ds).data_processor_attributes.is_none());
    assert!(owner(&ds).data_processor_tabular_sections.is_none());
    assert!(owner(&ds).data_processor_object_type.is_none());
}
