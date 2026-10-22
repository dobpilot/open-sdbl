use super::*;
const ID: &str = "c4a91334-1238-4808-8511-77875d713c00";
const RAW: &[u8] = include_bytes!("../../tests/fixtures/buh-data-load-sessions/owner.deflate");
const ATTRS: [&str; 2] = [
    "ca157e25-a955-41e9-981e-095f826b4bb7",
    "b744dd79-ac7b-4b30-9537-4362ad14b49d",
];
fn acquired() -> Vec<ConfigDescriptor> {
    super::super::parse_config_descriptors(ID, RAW).unwrap()
}
fn vals(v: &mut Value) -> &mut Vec<Value> {
    match v {
        Value::List(v) => v,
        _ => panic!("ожидается список"),
    }
}
fn rows(v: &mut Value) -> &mut Vec<Value> {
    vals(&mut vals(v)[6])
}
fn typed(v: &mut Value) -> &mut Vec<Value> {
    vals(&mut vals(&mut vals(&mut vals(v)[0])[1])[1])
}
fn identity(v: &mut Value) -> &mut Vec<Value> {
    vals(&mut typed(v)[1])
}
fn owner(ds: &[ConfigDescriptor]) -> &ConfigDescriptor {
    ds.iter().find(|d| d.object_guid.as_str() == ID).unwrap()
}
fn mutate(edit: impl FnOnce(&mut Value, &mut Vec<ConfigDescriptor>)) -> Vec<ConfigDescriptor> {
    let mut ds = acquired();
    for d in &mut ds {
        d.catalog_attributes = None;
        d.catalog_tabular_sections = None;
        d.declared_types = None;
    }
    let mut v =
        super::super::parse_serialized(&super::super::inflate_raw_deflate(RAW).unwrap()).unwrap();
    edit(&mut v, &mut ds);
    project(&v, &ID.parse().unwrap(), &mut ds);
    ds
}
#[test]
fn native_two_ordered_date_time_attributes_and_empty_sections_match_xml() {
    let ds = acquired();
    assert_eq!(
        owner(&ds)
            .catalog_attributes
            .as_ref()
            .unwrap()
            .iter()
            .map(|g| g.as_str())
            .collect::<Vec<_>>(),
        ATTRS
    );
    assert!(
        owner(&ds)
            .catalog_tabular_sections
            .as_ref()
            .unwrap()
            .is_empty()
    );
    for (id, name) in ATTRS.iter().zip(["ДатаНачала", "ДатаЗавершения"]) {
        let d = ds.iter().find(|d| d.object_guid.as_str() == *id).unwrap();
        assert_eq!(d.name, name);
        let ts = d.declared_types.as_ref().unwrap();
        assert_eq!(ts.len(), 1);
        assert_eq!(ts[0].tag, "D");
        assert!(ts[0].reference.is_none());
        assert!(ts[0].qualifiers.is_empty());
    }
}
#[test]
fn malformed_last_rows_and_owner_fail_closed_without_prefix() {
    for mode in 0..20 {
        let ds = mutate(|v, ds| match mode {
            0 => rows(v)[1] = Value::Atom("1".into()),
            1 => vals(&mut vals(rows(v).last_mut().unwrap())[0])[0] = Value::Atom("3".into()),
            2 => {
                vals(&mut vals(rows(v).last_mut().unwrap())[0]).pop();
            }
            3 => vals(&mut vals(rows(v).last_mut().unwrap())[0])[3] = Value::Atom("1".into()),
            4 => vals(&mut vals(rows(v).last_mut().unwrap())[0])[4] = Value::Atom("0".into()),
            5 => vals(&mut vals(rows(v).last_mut().unwrap())[0])[5] = Value::Atom("0".into()),
            6 => {
                identity(rows(v).last_mut().unwrap())[4] =
                    Value::String("неизмеренный комментарий".into())
            }
            7 => identity(rows(v).last_mut().unwrap())[4] = Value::Atom(String::new()),
            8 => identity(rows(v).last_mut().unwrap())[4] = Value::Null,
            9 => identity(rows(v).last_mut().unwrap())[3] = Value::Null,
            10 => identity(rows(v).last_mut().unwrap())[5] = Value::Atom("1".into()),
            11 => typed(rows(v).last_mut().unwrap())[2] = Value::Null,
            12 => {
                let id = identity(&mut rows(v)[2])[1].clone();
                identity(rows(v).last_mut().unwrap())[1] = id;
            }
            13 => {
                let id = identity(rows(v).last_mut().unwrap())[1].clone();
                let id = guid(&list(&id).unwrap()[2]).unwrap();
                ds.retain(|d| d.object_guid != id);
            }
            14 => {
                let d = ds
                    .iter()
                    .find(|d| d.object_guid.as_str() == ATTRS[1])
                    .unwrap()
                    .clone();
                ds.push(d);
            }
            15 => vals(&mut identity(rows(v).last_mut().unwrap())[1])[2] = Value::Atom(ID.into()),
            16 => vals(&mut vals(v)[1])[0] = Value::Atom("55".into()),
            17 => {
                vals(&mut vals(v)[1]).pop();
            }
            18 => {
                vals(&mut vals(&mut vals(&mut vals(v)[1])[9])[1])[4] =
                    Value::String("неизмеренный".into())
            }
            _ => vals(&mut vals(&mut vals(&mut vals(v)[1])[9])[1])[6] = Value::Atom("1".into()),
        });
        assert!(owner(&ds).catalog_attributes.is_none(), "mode {mode}");
        assert!(ds.iter().all(|d| d.declared_types.is_none()), "mode {mode}");
    }
}
#[test]
fn unmeasured_nonempty_sections_are_not_fabricated_empty() {
    let ds = mutate(|v, _| {
        let r = vals(&mut vals(v)[5]);
        r[1] = Value::Atom("1".into());
        r.push(Value::Null);
    });
    assert_eq!(owner(&ds).catalog_attributes.as_ref().unwrap().len(), 2);
    assert!(owner(&ds).catalog_tabular_sections.is_none());
}
