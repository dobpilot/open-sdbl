use super::*;

const OWNER: &str = "1055d15b-8cb5-4ff0-a526-7fd20a08a96c";
const COLLECTION: &str = "31182525-9346-4595-81f8-6f91a72ebe06";

fn parsed() -> Vec<ConfigDescriptor> {
    super::super::parse_config_descriptors(
        OWNER,
        include_bytes!("../../tests/fixtures/buh-characteristic-0919/owner.deflate"),
    )
    .unwrap()
}

fn owner(ds: &[ConfigDescriptor]) -> &ConfigDescriptor {
    ds.iter().find(|d| d.object_guid.as_str() == OWNER).unwrap()
}

fn source() -> Value {
    super::super::parse_serialized(include_bytes!(
        "../../tests/fixtures/buh-characteristic-0919/owner.config"
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
fn all_buh_characteristic_members_match_native_xml_order_and_qualified_types() {
    let ds = parsed();
    let ids = owner(&ds)
        .characteristic_attributes
        .as_ref()
        .expect("complete native characteristic collection");
    let expected =
        include_str!("../../tests/fixtures/buh-characteristic-0919/native-attributes.tsv")
            .lines()
            .map(|line| line.split('\t').collect::<Vec<_>>())
            .collect::<Vec<_>>();
    assert_eq!(ids.len(), 28);
    assert_eq!(ids.len(), expected.len());
    for (id, row) in ids.iter().zip(expected) {
        assert_eq!(id.as_str(), row[0]);
        let field = ds.iter().find(|d| &d.object_guid == id).unwrap();
        assert_eq!(field.name, row[1]);
        let types = field.declared_types.as_ref().unwrap();
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].tag, row[2]);
        assert_eq!(
            types[0].reference.as_ref().map(Guid::as_str).unwrap_or(""),
            row[3]
        );
        let qualifiers = if row[4].is_empty() {
            Vec::new()
        } else {
            row[4].split(',').map(|s| Value::Atom(s.into())).collect()
        };
        assert_eq!(types[0].qualifiers, qualifiers);
    }
}

#[test]
fn invalid_buh_characteristic_membership_never_returns_a_partial_list() {
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
                wrapper[0] = Value::Atom("3".into());
                wrapper.extend([Value::Atom("0".into()), Value::Atom("0".into())]);
            }
            _ => {
                rows(&mut v).pop();
            }
        }
        let mut ds = parsed();
        project(&v, &OWNER.parse().unwrap(), &mut ds);
        assert!(
            owner(&ds).characteristic_attributes.is_none(),
            "mode {mode}"
        );
        assert!(owner(&ds).characteristic_settings.is_some());
    }
}

#[test]
fn missing_or_ambiguous_characteristic_descriptors_keep_membership_unknown() {
    let v = source();
    for duplicate in [false, true] {
        let mut ds = parsed();
        let index = ds.iter().position(|d| d.name == "ВидСвойства").unwrap();
        if duplicate {
            ds.push(ds[index].clone());
        } else {
            ds.remove(index);
        }
        assert!(complete_attributes(list(&v).unwrap(), COLLECTION, &ds).is_none());
    }
}

#[test]
fn modern_characteristic_wrappers_keep_the_complete_ordered_membership() {
    let mut v = source();
    for row in rows(&mut v).iter_mut().skip(2) {
        let wrapper = values(&mut values(row)[0]);
        wrapper[0] = Value::Atom("3".into());
        wrapper.extend([Value::Atom("0".into()), Value::Atom("0".into())]);
    }
    let mut ds = parsed();
    project(&v, &OWNER.parse().unwrap(), &mut ds);
    let expected =
        include_str!("../../tests/fixtures/buh-characteristic-0919/native-attributes.tsv")
            .lines()
            .map(|line| line.split_once('\t').unwrap().0)
            .collect::<Vec<_>>();
    assert_eq!(
        owner(&ds)
            .characteristic_attributes
            .as_ref()
            .unwrap()
            .iter()
            .map(Guid::as_str)
            .collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn verified_empty_characteristic_membership_remains_known() {
    let mut v = source();
    rows(&mut v).truncate(2);
    rows(&mut v)[1] = Value::Atom("0".into());
    let mut ds = parsed();
    project(&v, &OWNER.parse().unwrap(), &mut ds);
    assert_eq!(owner(&ds).characteristic_attributes, Some(Vec::new()));
}
