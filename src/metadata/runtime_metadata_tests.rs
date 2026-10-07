use super::*;
const BP: &str = "6db4bbe0-ce4c-4389-ad25-9a10c1857192";
fn mutated_process(edit: impl FnOnce(&mut Value)) -> Vec<ConfigDescriptor> {
    let mut ds = super::super::parse_config_descriptors(
        BP,
        include_bytes!("../../tests/fixtures/runtime-object-099/tabular-section.native.deflate"),
    )
    .unwrap();
    let mut value = super::super::parse_serialized(include_bytes!(
        "../../tests/fixtures/runtime-object-099/tabular-section.native.config"
    ))
    .unwrap();
    edit(&mut value);
    for d in &mut ds {
        d.business_process_number = None;
        d.business_process_attributes = None;
        d.business_process_tabular_sections = None;
        d.business_process_standard_fill_checking = None;
        d.tabular_section_attributes = None;
    }
    project(&value, &BP.parse().unwrap(), &mut ds);
    ds
}
fn values(v: &mut Value) -> &mut Vec<Value> {
    match v {
        Value::List(v) => v,
        _ => panic!("native list expected"),
    }
}
fn owner(ds: &[ConfigDescriptor]) -> &ConfigDescriptor {
    ds.iter().find(|d| d.object_guid.as_str() == BP).unwrap()
}
#[test]
fn unknown_process_number_codes_do_not_erase_independently_complete_members() {
    for index in [16, 18, 40] {
        let ds = mutated_process(|v| {
            values(&mut values(v)[1])[index] =
                Value::Atom(if index == 18 { "unknown" } else { "999" }.into())
        });
        let d = owner(&ds);
        assert!(d.business_process_number.is_none());
        assert_eq!(d.business_process_attributes.as_ref().unwrap().len(), 2);
    }
}
#[test]
fn standard_table_rejects_unknown_codes_duplicates_and_sparse_members() {
    for mode in 0..4 {
        let ds = mutated_process(|v| {
            let table = &mut values(&mut values(v)[1])[30];
            let rows = values(&mut values(table)[1]);
            match mode {
                0 => rows[1] = Value::Atom("6".into()),
                1 => rows[5] = rows[2].clone(),
                2 => {
                    let bag = values(&mut rows[4]);
                    let position = bag
                        .iter()
                        .position(|v| atom(v) == Some("2723eb98-b4c1-498a-a6f3-70444757902f"))
                        .unwrap();
                    values(&mut bag[position + 1])[2] = Value::Atom("999".into());
                }
                3 => {
                    rows.pop();
                }
                _ => unreachable!(),
            }
        });
        assert!(owner(&ds).business_process_standard_fill_checking.is_none());
        assert_eq!(
            owner(&ds)
                .business_process_attributes
                .as_ref()
                .unwrap()
                .len(),
            2
        );
    }
}
#[test]
fn malformed_section_and_attribute_wrappers_never_publish_partial_membership() {
    for section in [false, true] {
        let ds = mutated_process(|v| {
            let id = if section {
                "a3fe6537-d787-40f7-8a06-419d2f0c1cfd"
            } else {
                "87c988de-ecbf-413b-87b0-b9516df05e28"
            };
            let collection = values(v)
                .iter_mut()
                .find(|v| list(v).and_then(|l| l.first()).and_then(atom) == Some(id))
                .unwrap();
            let row = &mut values(collection)[2];
            if section {
                values(&mut values(row)[0])[1] = Value::Atom("unknown".into());
            } else {
                values(&mut values(row)[0])[0] = Value::Atom("unknown".into());
            }
        });
        let d = owner(&ds);
        if section {
            assert!(d.business_process_tabular_sections.is_none());
        } else {
            assert!(d.business_process_attributes.is_none());
        }
    }
}

#[test]
fn accumulation_unknown_kind_and_corrupt_members_preserve_independent_collections() {
    const ID: &str = "c72d739a-05e3-4e9e-bcc7-3bf8baf5dc63";
    for mode in 0..4 {
        let mut ds = super::super::parse_config_descriptors(
            ID,
            include_bytes!("../../tests/fixtures/runtime-object-099/ОстаткиПорядок.deflate"),
        )
        .unwrap();
        let mut value = super::super::parse_serialized(include_bytes!(
            "../../tests/fixtures/runtime-object-099/ОстаткиПорядок.config"
        ))
        .unwrap();
        if mode == 0 {
            values(&mut values(&mut value)[1])[15] = Value::Atom("999".into());
        } else {
            let collection = values(&mut value)
                .iter_mut()
                .find(|v| {
                    list(v).and_then(|v| v.first()).and_then(atom)
                        == Some("b64d9a43-1642-11d6-a3c7-0050bae0a776")
                })
                .unwrap();
            let rows = values(collection);
            match mode {
                1 => rows[1] = Value::Atom("3".into()),
                2 => rows[3] = rows[2].clone(),
                3 => {
                    values(&mut values(&mut rows[2])[0])[0] = Value::Atom("unknown".into());
                }
                _ => unreachable!(),
            }
        }
        project(&value, &ID.parse().unwrap(), &mut ds);
        let d = ds.iter().find(|d| d.object_guid.as_str() == ID).unwrap();
        assert_eq!(d.accumulation_register_resources.as_ref().unwrap().len(), 2);
        if mode == 0 {
            assert!(d.accumulation_register_kind.is_none());
            assert_eq!(
                d.accumulation_register_dimensions.as_ref().unwrap().len(),
                2
            );
        } else {
            assert!(d.accumulation_register_dimensions.is_none());
            assert_eq!(
                d.accumulation_register_kind,
                Some(AccumulationRegisterKind::Balance)
            );
        }
    }
}

#[test]
fn unresolved_named_types_invalidate_process_owner_and_section_membership() {
    for section in [false, true] {
        let mut ds = mutated_process(|_| {});
        let member = if section {
            "ТекстСтроки"
        } else {
            "ТекстПробы"
        };
        let definition_id: Guid = "95691e8c-6f02-43b4-9bcc-22d6111f877f".parse().unwrap();
        let target = ds.iter_mut().find(|d| d.name == member).unwrap();
        target.declared_types = Some(vec![ConfigDeclaredType {
            tag: "#".into(),
            reference: Some(definition_id.clone()),
            qualifiers: vec![],
        }]);
        let mut definition = target.clone();
        definition.object_guid = definition_id.clone();
        definition.defined_type_id = Some(definition_id);
        definition.declared_types = None;
        ds.push(definition);
        resolve_named_types(&mut ds);
        if section {
            assert!(
                ds.iter()
                    .find(|d| d.name == "СтрокиПробы")
                    .unwrap()
                    .tabular_section_attributes
                    .is_none()
            );
        } else {
            assert!(owner(&ds).business_process_attributes.is_none());
        }
    }
}

#[test]
fn unknown_section_wrappers_do_not_publish_owner_membership() {
    for mode in 0..4 {
        let ds = mutated_process(|v| {
            let collection = values(v)
                .iter_mut()
                .find(|v| {
                    list(v).and_then(|v| v.first()).and_then(atom)
                        == Some("a3fe6537-d787-40f7-8a06-419d2f0c1cfd")
                })
                .unwrap();
            let row = &mut values(collection)[2];
            match mode {
                0 => values(&mut values(row)[0])[0] = Value::Atom("unknown".into()),
                1 => {
                    values(&mut values(row)[0]).pop();
                }
                2 => values(&mut values(&mut values(row)[0])[1])[0] = Value::Atom("unknown".into()),
                3 => {
                    values(&mut values(&mut values(row)[0])[1]).pop();
                }
                _ => unreachable!(),
            }
        });
        assert!(owner(&ds).business_process_tabular_sections.is_none());
        assert_eq!(
            owner(&ds)
                .business_process_attributes
                .as_ref()
                .unwrap()
                .len(),
            2
        );
    }
}
