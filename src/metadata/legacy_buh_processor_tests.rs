use super::*;
const ID: &str = "c58a200f-65a6-4130-a99d-42995cfb38ea";
const RAW: &[u8] = include_bytes!(
    "../../tests/fixtures/buh-vat-processor-0918/c58a200f-65a6-4130-a99d-42995cfb38ea.deflate"
);
fn acquired() -> Vec<ConfigDescriptor> {
    super::super::parse_config_descriptors(ID, RAW).unwrap()
}
fn owner(ds: &[ConfigDescriptor]) -> &ConfigDescriptor {
    ds.iter().find(|d| d.object_guid.as_str() == ID).unwrap()
}
#[test]
fn native_legacy_buh_processor_preserves_complete_ordered_members_and_types() {
    let ds = acquired();
    let own = owner(&ds);
    assert_eq!(own.name, "СверкаДанныхУчетаНДС");
    assert_eq!(
        own.data_processor_object_type.as_ref().unwrap().as_str(),
        "3198584b-240f-4908-b581-04a26dcfa2b7"
    );
    let xml_members = include_str!("../../tests/fixtures/buh-vat-processor-0918/xml-members.txt");
    let mut attributes = Vec::new();
    let mut sections = Vec::new();
    let mut fields = Vec::new();
    for row in xml_members.lines() {
        let (identity, name) = row.split_once("; name=").unwrap();
        let (kind, id) = identity.split_once('=').unwrap();
        let d = ds.iter().find(|d| d.object_guid.as_str() == id).unwrap();
        assert_eq!(d.name, name);
        match kind {
            "attribute" => attributes.push(d.object_guid.clone()),
            "section" => sections.push(d.object_guid.clone()),
            "section.attribute" => fields.push(d.object_guid.clone()),
            _ => panic!(),
        }
    }
    assert_eq!(own.data_processor_attributes.as_ref(), Some(&attributes));
    assert_eq!(
        own.data_processor_tabular_sections.as_ref(),
        Some(&sections)
    );
    let section = ds.iter().find(|d| d.object_guid == sections[0]).unwrap();
    assert_eq!(section.tabular_section_attributes.as_ref(), Some(&fields));
    let expected = [
        (
            "d335af30-73f9-450d-b53b-31f71542b51e",
            "#",
            "c92a1124-c41e-42a4-bda7-3bbac9c6039e",
            "",
        ),
        (
            "fdff4019-ddeb-40c8-8b94-2dcdb330f42e",
            "#",
            "9f6206b2-1ed6-423c-9b08-fd4978930c49",
            "",
        ),
        ("1025b5ea-1143-41f2-906b-6171537dcc84", "D", "", "D"),
        (
            "e91c170f-5f64-4ad5-83db-0cbcd1ab3b72",
            "#",
            "d7be9875-8f30-4b48-8ce8-82daa6e416ce",
            "",
        ),
        ("b644d7f9-8858-42df-bd7a-1e8403479757", "B", "", ""),
        (
            "e6caab54-aa68-4c2b-9b24-b16ad46bcde2",
            "#",
            "9f6206b2-1ed6-423c-9b08-fd4978930c49",
            "",
        ),
        ("2d19966e-973c-4592-96fb-af6ae4ac6388", "S", "", "50,1"),
        ("08ab9a2f-0c5f-47bf-ac63-3e9a45a6290c", "S", "", "100,1"),
        ("d242c220-0780-426f-8293-ec2a6771effa", "D", "", "D"),
        (
            "4668d509-86f6-4986-ad9c-c9855116d16e",
            "#",
            "5fa17bff-daa5-408d-87dd-5fc78dc544be",
            "",
        ),
        ("69320438-6dcb-4e57-81d4-708baf787fa7", "N", "", "15,2,1"),
        ("415122f1-4954-46fd-a121-c3c35aa3ad4b", "N", "", "15,2,1"),
        ("9fc6b350-c7d7-4c92-ab45-9475fce02010", "S", "", "36,1"),
        ("a1a20910-358f-47c3-bdf3-045cd66cdfbc", "N", "", "1,0,1"),
        ("9bc69426-f332-4b58-814f-2f67ff9c79d3", "S", "", ""),
    ];
    for (id, tag, reference, qualifiers) in expected {
        let types = ds
            .iter()
            .find(|d| d.object_guid.as_str() == id)
            .unwrap()
            .declared_types
            .as_ref()
            .unwrap();
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].tag, tag);
        assert_eq!(
            types[0].reference.as_ref().map(Guid::as_str).unwrap_or(""),
            reference
        );
        assert_eq!(
            types[0]
                .qualifiers
                .iter()
                .map(|v| v.as_string().or_else(|| atom(v)).unwrap())
                .collect::<Vec<_>>()
                .join(","),
            qualifiers
        );
    }
}

fn values(v: &mut Value) -> &mut Vec<Value> {
    match v {
        Value::List(v) => v,
        _ => panic!("native list expected"),
    }
}
fn mutated(edit: impl FnOnce(&mut Value, &mut Vec<ConfigDescriptor>)) -> Vec<ConfigDescriptor> {
    let text = super::super::inflate_raw_deflate(RAW).unwrap();
    let mut value = super::super::parse_serialized(&text).unwrap();
    let mut ds = acquired();
    for d in &mut ds {
        d.data_processor_attributes = None;
        d.data_processor_tabular_sections = None;
        d.data_processor_object_type = None;
        d.tabular_section_attributes = None;
        d.declared_types = None;
    }
    edit(&mut value, &mut ds);
    project(&value, &ID.parse().unwrap(), &mut ds);
    ds
}
#[test]
fn legacy_owner_validation_does_not_accept_unmeasured_headers() {
    for mode in 0..5 {
        let ds = mutated(|v, _| {
            let header = values(&mut values(&mut values(&mut values(v)[1])[3])[1]);
            match mode {
                0 => header[0] = Value::Atom("2".into()),
                1 => {
                    header.pop();
                }
                2 => values(&mut header[1])[0] = Value::Atom("1".into()),
                3 => header[2] = Value::String(String::new()),
                4 => header[5] = Value::Atom("1".into()),
                _ => unreachable!(),
            }
        });
        assert!(owner(&ds).data_processor_attributes.is_none());
        assert!(owner(&ds).data_processor_tabular_sections.is_none());
        assert!(owner(&ds).data_processor_object_type.is_none());
    }
}
#[test]
fn malformed_legacy_collections_do_not_publish_a_prefix() {
    for mode in 0..6 {
        let ds = mutated(|v, ds| {
            let rows = values(&mut values(v)[7]);
            match mode {
                0 => {
                    rows.pop();
                }
                1 => rows[5] = rows[2].clone(),
                2 => {
                    values(&mut values(&mut rows[5])[0]).pop();
                }
                3 => {
                    ds.retain(|d| d.name != "УчетнаяЗапись");
                }
                4 => {
                    let d = ds
                        .iter()
                        .find(|d| d.name == "УчетнаяЗапись")
                        .unwrap()
                        .clone();
                    ds.push(d);
                }
                5 => {
                    let typed = values(&mut values(&mut values(&mut rows[5])[0])[1]);
                    typed[1] = Value::Null;
                }
                _ => unreachable!(),
            }
        });
        assert!(
            owner(&ds).data_processor_attributes.is_none(),
            "mode {mode}"
        );
        assert_eq!(
            owner(&ds)
                .data_processor_tabular_sections
                .as_ref()
                .unwrap()
                .len(),
            1
        );
        assert!(
            ds.iter()
                .find(|d| d.name == "Организация")
                .unwrap()
                .declared_types
                .is_none()
        );
    }
}
#[test]
fn malformed_legacy_section_or_fields_remain_independently_unknown() {
    for mode in 0..5 {
        let ds = mutated(|v, _| {
            let section = &mut values(&mut values(v)[3])[2];
            match mode {
                0 => {
                    values(section).pop();
                }
                1 => {
                    let c = values(&mut values(section)[2]);
                    c.pop();
                }
                2 => {
                    let c = values(&mut values(section)[2]);
                    c[12] = c[2].clone();
                }
                3 => {
                    let body = values(&mut values(&mut values(&mut values(section)[0])[1])[5]);
                    values(&mut body[1])[0] = Value::Atom("999".into());
                }
                4 => {
                    let c = values(&mut values(section)[2]);
                    let body = values(&mut values(&mut values(&mut c[12])[0])[1]);
                    let typed = values(&mut body[1]);
                    typed[2] = Value::List(vec![Value::String("Pattern".into()), Value::Null]);
                }
                _ => unreachable!(),
            }
        });
        assert_eq!(
            owner(&ds).data_processor_attributes.as_ref().unwrap().len(),
            4
        );
        if mode == 0 || mode == 3 {
            assert!(owner(&ds).data_processor_tabular_sections.is_none());
        } else {
            assert_eq!(
                owner(&ds)
                    .data_processor_tabular_sections
                    .as_ref()
                    .unwrap()
                    .len(),
                1
            );
        }
        assert!(
            ds.iter()
                .find(|d| d.name == "ЗапросыПоставщикам")
                .unwrap()
                .tabular_section_attributes
                .is_none()
        );
    }
}
