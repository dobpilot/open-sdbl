use super::*;

const OWNER: &str = "2633b0d5-2818-4fa0-abe9-6e801e63c9c7";
const COLLECTION: &str = "45e46cbc-3e24-4165-8b7b-cc98a6f80211";

fn parsed() -> Vec<ConfigDescriptor> {
    super::super::parse_config_descriptors(
        OWNER,
        include_bytes!("../../tests/fixtures/buh-credit-document-0920/owner.deflate"),
    )
    .unwrap()
}

fn owner(ds: &[ConfigDescriptor]) -> &ConfigDescriptor {
    ds.iter().find(|d| d.object_guid.as_str() == OWNER).unwrap()
}

fn source() -> Value {
    super::super::parse_serialized(include_bytes!(
        "../../tests/fixtures/buh-credit-document-0920/owner.config"
    ))
    .unwrap()
}

fn values(v: &mut Value) -> &mut Vec<Value> {
    match v {
        Value::List(v) => v,
        _ => panic!("native list expected"),
    }
}

fn rows(v: &mut Value) -> &mut Vec<Value> {
    values(v)
        .iter_mut()
        .find(|v| list(v).and_then(|v| v.first()).and_then(atom) == Some(COLLECTION))
        .map(values)
        .unwrap()
}

#[test]
fn all_buh_document_members_match_native_xml_order_and_qualified_types() {
    let ds = parsed();
    let ids = owner(&ds)
        .document_attributes
        .as_ref()
        .expect("complete native document collection");
    let expected =
        include_str!("../../tests/fixtures/buh-credit-document-0920/native-attributes.tsv")
            .lines()
            .map(|line| line.split('\t').collect::<Vec<_>>())
            .collect::<Vec<_>>();
    assert_eq!(ids.len(), 23);
    assert_eq!(ids.len(), expected.len());
    for (id, row) in ids.iter().zip(expected) {
        assert_eq!(id.as_str(), row[0]);
        let field = ds.iter().find(|d| &d.object_guid == id).unwrap();
        assert_eq!(field.name, row[1]);
        let types = field
            .declared_types
            .as_ref()
            .expect("native qualified type");
        assert!(!types.is_empty());
    }
}

#[test]
fn invalid_buh_document_membership_never_returns_a_partial_list() {
    for mode in 0..8 {
        let mut v = source();
        match mode {
            0 => rows(&mut v)[1] = Value::Atom("27".into()),
            1 => values(&mut values(&mut rows(&mut v)[2])[0])[0] = Value::Atom("unknown".into()),
            2 => {
                values(&mut values(&mut rows(&mut v)[2])[0]).pop();
            }
            3 => {
                let copy = rows(&mut v)[2].clone();
                rows(&mut v)[3] = copy;
            }
            4 => {
                let wrapper = values(&mut values(&mut rows(&mut v)[2])[0]);
                let body = values(&mut wrapper[1]);
                values(&mut body[1])[2] = Value::List(vec![
                    Value::String("Pattern".into()),
                    Value::List(vec![Value::String("unknown".into())]),
                ]);
            }
            5 => {
                let wrapper = values(&mut values(&mut rows(&mut v)[2])[0]);
                let body = values(&mut wrapper[1]);
                values(&mut values(&mut body[1])[1])[0] = Value::Atom("missing".into());
            }
            6 => {
                let wrapper = values(&mut values(&mut rows(&mut v)[2])[0]);
                wrapper[0] = Value::Atom("unknown".into());
                wrapper.extend([Value::Atom("0".into()), Value::Atom("0".into())]);
            }
            _ => {
                rows(&mut v).pop();
            }
        }
        let mut ds = parsed();
        project(&v, &OWNER.parse().unwrap(), &mut ds);
        assert!(owner(&ds).document_attributes.is_none(), "mode {mode}");
    }
}

#[test]
fn missing_or_ambiguous_document_descriptors_keep_membership_unknown() {
    let v = source();
    for duplicate in [false, true] {
        let mut ds = parsed();
        let index = ds.iter().position(|d| d.name == "Состояние").unwrap();
        if duplicate {
            ds.push(ds[index].clone());
        } else {
            ds.remove(index);
        }
        project(&v, &OWNER.parse().unwrap(), &mut ds);
        assert!(owner(&ds).document_attributes.is_none());
    }
}

#[test]
fn modern_document_wrappers_keep_the_complete_ordered_membership() {
    let mut v = source();
    for row in rows(&mut v).iter_mut().skip(2) {
        let wrapper = values(&mut values(row)[0]);
        wrapper[0] = Value::Atom("6".into());
        wrapper.extend([Value::Atom("0".into()), Value::Atom("0".into())]);
    }
    let root = values(&mut v);
    let class = values(&mut root[1]);
    let identity = values(&mut values(&mut class[9])[1]);
    identity[0] = Value::Atom("3".into());
    identity.push(Value::Atom("0".into()));
    for row in rows(&mut v).iter_mut().skip(2) {
        let wrapper = values(&mut values(row)[0]);
        let body = values(&mut wrapper[1]);
        let typed = values(&mut body[1]);
        let identity = values(&mut typed[1]);
        identity[0] = Value::Atom("3".into());
        identity.push(Value::Atom("0".into()));
    }
    let mut ds = parsed();
    project(&v, &OWNER.parse().unwrap(), &mut ds);
    let expected =
        include_str!("../../tests/fixtures/buh-credit-document-0920/native-attributes.tsv")
            .lines()
            .map(|line| line.split_once('\t').unwrap().0)
            .collect::<Vec<_>>();
    assert_eq!(
        owner(&ds)
            .document_attributes
            .as_ref()
            .unwrap()
            .iter()
            .map(Guid::as_str)
            .collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn verified_empty_document_membership_remains_known() {
    let mut v = source();
    rows(&mut v).truncate(2);
    rows(&mut v)[1] = Value::Atom("0".into());
    let mut ds = parsed();
    project(&v, &OWNER.parse().unwrap(), &mut ds);
    assert_eq!(owner(&ds).document_attributes, Some(Vec::new()));
}

#[test]
fn native_document_object_and_reference_identities_are_preserved() {
    // Ожидания взяты из GeneratedType Object/Ref парной нативной XML.
    let xml = include_str!("../../tests/fixtures/buh-credit-document-0920/Document.xml");
    let source = source();
    for (index, expected) in [
        (1, "7522f75a-a619-46f3-8591-9e6e9f0c85e9"),
        (2, "21925c77-d393-4a82-9951-7eed3fb3ec58"),
        (3, "80f066e2-6ea4-4cf0-a3fa-5fc343dd8e5a"),
        (4, "2f2097f0-7caa-42ee-92cc-f2725d4ba809"),
    ] {
        assert!(xml.contains(expected));
        assert_eq!(at(&source, &[1, index]).and_then(atom), Some(expected));
    }
    let ds = parsed();
    let owner = owner(&ds);
    assert_eq!(owner.resource_guid.as_str(), OWNER);
    assert_eq!(owner.object_guid.as_str(), OWNER);
    assert_eq!(
        owner.object_reference_type.as_ref().map(Guid::as_str),
        Some("80f066e2-6ea4-4cf0-a3fa-5fc343dd8e5a")
    );
}

#[test]
fn an_unmeasured_document_owner_identity_does_not_enable_attribute_projection() {
    let mut source = source();
    let class = values(&mut values(&mut source)[1]);
    values(&mut values(&mut class[9])[1])
        .extend([Value::Atom("0".into()), Value::Atom("0".into())]);
    let mut ds = parsed();
    owner_mut(&mut ds).document_attributes = None;
    project(&source, &OWNER.parse().unwrap(), &mut ds);
    assert!(owner(&ds).document_attributes.is_none());
}

fn owner_mut(ds: &mut [ConfigDescriptor]) -> &mut ConfigDescriptor {
    ds.iter_mut()
        .find(|d| d.object_guid.as_str() == OWNER)
        .unwrap()
}

#[test]
fn modern_document_identity_accepts_the_measured_older_attribute_wrapper() {
    let mut source = source();
    let class = values(&mut values(&mut source)[1]);
    let identity = values(&mut values(&mut class[9])[1]);
    identity[0] = Value::Atom("3".into());
    identity.push(Value::Atom("0".into()));
    for row in rows(&mut source).iter_mut().skip(2) {
        let wrapper = values(&mut values(row)[0]);
        let body = values(&mut wrapper[1]);
        let identity = values(&mut values(&mut body[1])[1]);
        identity[0] = Value::Atom("3".into());
        identity.push(Value::Atom("0".into()));
    }
    let mut ds = parsed();
    project(&source, &OWNER.parse().unwrap(), &mut ds);
    assert_eq!(
        owner(&ds).document_attributes.as_ref().map(Vec::len),
        Some(23)
    );
}

#[test]
fn the_known_document_class_retains_existing_native_number_and_empty_memberships() {
    let ds = parsed();
    let descriptor = owner(&ds);
    assert_eq!(
        descriptor.document_number,
        Some(DocumentNumberSettings {
            number_type: CatalogCodeType::String,
            length: 9,
            variable_length: true,
        })
    );
    assert_eq!(
        descriptor.document_numbering,
        Some(DocumentNumberingSettings {
            periodicity: DocumentNumberPeriodicity::Nonperiodical,
            numerator: None,
        })
    );
    assert_eq!(descriptor.document_registers, Some(Vec::new()));
    assert_eq!(descriptor.document_tabular_sections, Some(Vec::new()));
}
