use super::*;
const OWNER: &str = "dad11c2e-08fc-4a6b-8829-8be6c64c15fc";
const COLLECTION: &str = "87c988de-ecbf-413b-87b0-b9516df05e28";
fn parsed() -> Vec<ConfigDescriptor> {
    super::super::parse_config_descriptors(
        OWNER,
        include_bytes!("../../tests/fixtures/business-process-wrapper2/owner.deflate"),
    )
    .unwrap()
}
fn owner(ds: &[ConfigDescriptor]) -> &ConfigDescriptor {
    ds.iter().find(|d| d.object_guid.as_str() == OWNER).unwrap()
}
fn values(v: &mut Value) -> &mut Vec<Value> {
    match v {
        Value::List(v) => v,
        _ => panic!("native list expected"),
    }
}
fn source() -> Value {
    super::super::parse_serialized(include_bytes!(
        "../../tests/fixtures/business-process-wrapper2/owner.config"
    ))
    .unwrap()
}
fn rows(v: &mut Value) -> &mut Vec<Value> {
    values(v)
        .iter_mut()
        .find(|v| list(v).and_then(|v| v.first()).and_then(atom) == Some(COLLECTION))
        .map(values)
        .unwrap()
}
#[test]
fn all_legacy_business_process_members_match_the_complete_native_xml() {
    let ds = parsed();
    let d = owner(&ds);
    let ids = d
        .business_process_attributes
        .as_ref()
        .expect("complete native collection");
    let expected =
        include_str!("../../tests/fixtures/business-process-wrapper2/native-attributes.tsv")
            .lines()
            .map(|line| line.split('\t').collect::<Vec<_>>())
            .collect::<Vec<_>>();
    assert_eq!(ids.len(), 27);
    assert_eq!(ids.len(), expected.len());
    for (id, row) in ids.iter().zip(expected) {
        assert_eq!(id.as_str(), row[0]);
        let field = ds.iter().find(|d| &d.object_guid == id).unwrap();
        assert_eq!(field.name, row[1]);
        assert_eq!(
            field.declared_types.as_ref().unwrap().len(),
            row[2].parse::<usize>().unwrap()
        );
    }
    assert!(d.business_process_number.is_some());
    assert!(d.business_process_tabular_sections.is_some());
}
#[test]
fn legacy_unknown_mixed_and_duplicate_rows_do_not_publish_partial_membership() {
    for mode in 0..5 {
        let mut v = source();
        let row = &mut rows(&mut v)[2];
        let wrapper = values(&mut values(row)[0]);
        match mode {
            0 => wrapper[0] = Value::Atom("unknown".into()),
            1 => {
                wrapper.pop();
            }
            2 => {
                wrapper[0] = Value::Atom("3".into());
                wrapper.push(Value::Atom("0".into()));
                wrapper.push(Value::Atom("0".into()));
            }
            3 => {
                let copy = rows(&mut v)[2].clone();
                rows(&mut v)[3] = copy;
            }
            _ => {
                let body = values(&mut wrapper[1]);
                values(&mut body[1])[2] = Value::List(vec![
                    Value::String("Pattern".into()),
                    Value::List(vec![Value::String("unknown".into())]),
                ]);
            }
        }
        let mut ds = parsed();
        project(&v, &OWNER.parse().unwrap(), &mut ds);
        assert!(
            owner(&ds).business_process_attributes.is_none(),
            "mode {mode}"
        );
        assert!(owner(&ds).business_process_number.is_some());
    }
}
#[test]
fn a_missing_or_ambiguous_descriptor_keeps_the_collection_unknown() {
    let v = source();
    let root = list(&v).unwrap();
    let mut ds = parsed();
    let member: Guid = "a76b1fd1-ba00-40e6-8756-f17fee449818".parse().unwrap();
    let field = ds.iter().find(|d| d.object_guid == member).unwrap().clone();
    ds.push(field);
    assert!(complete_attributes(root, COLLECTION, &ds).is_none());
    ds.retain(|d| d.object_guid != member);
    assert!(complete_attributes(root, COLLECTION, &ds).is_none());
}
