use super::*;
const ID: &str = "cc906f05-5ea6-43cb-a084-c4ee51ca4bd5";
const RAW: &[u8] = include_bytes!("../../tests/fixtures/buh-separation-constant/owner.deflate");
#[test]
fn native_boolean_constant_and_both_conditional_links_match_xml() {
    let ds = super::super::parse_config_descriptors(ID, RAW).unwrap();
    let d = ds.iter().find(|d| d.object_guid.as_str() == ID).unwrap();
    assert_eq!(d.name, "ИспользоватьРазделениеПоОбластямДанных");
    let t = d.declared_types.as_ref().expect("полный нативный Boolean");
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].tag, "B");
    assert!(t[0].reference.is_none());
    assert!(t[0].qualifiers.is_empty());
    for (id, raw) in [
        ("530a3164-4ef1-4b3b-8269-13764ef4bf15", include_bytes!("../../tests/fixtures/buh-separation-constant/530a3164-4ef1-4b3b-8269-13764ef4bf15.deflate").as_slice()),
        ("6df2bb92-558c-4453-9de4-e4176e8f93dc", include_bytes!("../../tests/fixtures/buh-separation-constant/6df2bb92-558c-4453-9de4-e4176e8f93dc.deflate").as_slice()),
    ] {
        let ds = super::super::parse_config_descriptors(id, raw).unwrap();
        let d = ds.iter().find(|d|d.object_guid.as_str()==id).unwrap();
        assert_eq!(d.separation.as_ref().unwrap().conditional_separation.as_ref().unwrap().as_str(), ID);
    }
}

fn values(v: &mut Value) -> &mut Vec<Value> {
    match v {
        Value::List(v) => v,
        _ => panic!("ожидается список"),
    }
}
fn at_mut<'a>(mut v: &'a mut Value, path: &[usize]) -> &'a mut Value {
    for i in path {
        v = &mut values(v)[*i];
    }
    v
}
#[test]
fn malformed_constant_profiles_publish_nothing() {
    let ds = super::super::parse_config_descriptors(ID, RAW).unwrap();
    let source =
        super::super::parse_serialized(&super::super::inflate_raw_deflate(RAW).unwrap()).unwrap();
    let identity = &[1, 1, 1, 1][..];
    let modes = 33;
    for mode in 0..modes {
        let mut v = source.clone();
        let mut target = ds.clone();
        for d in &mut target {
            d.declared_types = None;
        }
        match mode {
            0 => {
                values(&mut v).pop();
            }
            1 => {
                values(at_mut(&mut v, &[1])).pop();
            }
            2 => *at_mut(&mut v, &[1, 0]) = Value::Atom("15".into()),
            3 => {
                values(at_mut(&mut v, &[1, 1])).pop();
            }
            4 => *at_mut(&mut v, &[1, 1, 0]) = Value::Atom("28".into()),
            5 => {
                values(at_mut(&mut v, &[1, 1, 1])).pop();
            }
            6 => *at_mut(&mut v, &[1, 1, 1, 0]) = Value::Atom("3".into()),
            7 => {
                values(at_mut(&mut v, identity)).pop();
            }
            8 => *at_mut(&mut v, &[1, 1, 1, 1, 0]) = Value::Atom("3".into()),
            9 => {
                *at_mut(&mut v, &[1, 1, 1, 1, 1, 2]) =
                    Value::Atom("00000000-0000-0000-0000-000000000000".into())
            }
            10 => {
                *at_mut(&mut v, &[1, 1, 1, 1, 1, 2]) =
                    Value::Atom("530a3164-4ef1-4b3b-8269-13764ef4bf15".into())
            }
            11 => *at_mut(&mut v, &[1, 1, 1, 1, 4]) = Value::Atom("".into()),
            12 => *at_mut(&mut v, &[1, 1, 1, 1, 4]) = Value::Atom("bad".into()),
            13 => *at_mut(&mut v, &[1, 1, 1, 1, 4]) = Value::Null,
            14 => *at_mut(&mut v, &[1, 1, 1, 1, 4]) = Value::List(vec![]),
            15 => *at_mut(&mut v, &[1, 1, 1, 1, 5]) = Value::Atom("1".into()),
            16 => *at_mut(&mut v, &[1, 1, 1, 1, 3]) = Value::Null,
            17 => *at_mut(&mut v, &[1, 1, 1, 2]) = Value::Null,
            18 => *at_mut(&mut v, &[1, 1, 1, 2, 1, 0]) = Value::Atom("Unknown".into()),
            19 => {
                values(at_mut(&mut v, &[1, 1, 1, 2])).pop();
            }
            20 => values(at_mut(&mut v, &[1, 1, 1, 2]))
                .push(Value::List(vec![Value::Atom("B".into())])),
            21 => values(at_mut(&mut v, &[1, 1, 1, 2, 1])).push(Value::Atom("1".into())),
            22 => {
                let d = target[0].clone();
                target.push(d);
            }
            23 => target.clear(),
            24 => *at_mut(&mut v, &[1, 6]) = Value::Atom("0".into()),
            25 => *at_mut(&mut v, &[1, 1, 22]) = Value::Atom("1".into()),
            26 => *at_mut(&mut v, &[1, 1, 19, 1]) = Value::Null,
            27 => *at_mut(&mut v, &[1, 8]) = Value::List(vec![]),
            28 => *at_mut(&mut v, &[1, 10]) = Value::Atom(ID.into()),
            29 => *at_mut(&mut v, &[1, 1, 1, 1, 4]) = Value::String("unknown comment".into()),
            30 => *at_mut(&mut v, &[1, 1, 1, 2, 1, 0]) = Value::Atom("S".into()),
            31 => {
                *at_mut(&mut v, &[1, 2]) =
                    Value::Atom("00000000-0000-0000-0000-000000000000".into())
            }
            32 => *at_mut(&mut v, &[1, 11]) = Value::Atom("1".into()),
            _ => unreachable!(),
        }
        let before = format!("{target:?}");
        project(&v, &ID.parse().unwrap(), &mut target);
        assert_eq!(format!("{target:?}"), before, "режим {mode}");
    }
}
