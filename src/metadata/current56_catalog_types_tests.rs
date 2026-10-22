use super::*;
const ID: &str = "4bb0f7c3-62f3-4352-9bc8-e243dd18fe4a";
const ATTRS: [&str; 2] = [
    "bc106ef9-3eeb-40d4-aa59-ae1855ba1744",
    "73d8b319-9bea-4638-a602-23c12f37c227",
];
const RAW: &[u8] = include_bytes!("../../tests/fixtures/buh-extension-version-types/owner.deflate");
fn acquired() -> Vec<ConfigDescriptor> {
    super::super::parse_config_descriptors(ID, RAW).unwrap()
}
fn owner(ds: &[ConfigDescriptor]) -> &ConfigDescriptor {
    ds.iter().find(|d| d.object_guid.as_str() == ID).unwrap()
}
fn vals(v: &mut Value) -> &mut Vec<Value> {
    match v {
        Value::List(v) => v,
        _ => panic!("ожидается список"),
    }
}
fn own(v: &mut Value) -> &mut Vec<Value> {
    vals(&mut vals(v)[6])
}
fn wrapper(v: &mut Value) -> &mut Vec<Value> {
    vals(&mut vals(v)[0])
}
fn typed(v: &mut Value) -> &mut Vec<Value> {
    vals(&mut vals(&mut wrapper(v)[1])[1])
}
fn identity(v: &mut Value) -> &mut Vec<Value> {
    vals(&mut typed(v)[1])
}
fn owner_identity(v: &mut Value) -> &mut Vec<Value> {
    vals(&mut vals(&mut vals(&mut vals(v)[1])[9])[1])
}
#[test]
fn native_catalog_keeps_membership_and_matches_both_xml_types() {
    let ds = acquired();
    let o = owner(&ds);
    assert_eq!(
        o.catalog_attributes
            .as_ref()
            .unwrap()
            .iter()
            .map(|id| id.as_str())
            .collect::<Vec<_>>(),
        ATTRS
    );
    assert!(o.catalog_standard_attributes.is_some());
    assert!(o.catalog_tabular_sections.as_ref().unwrap().is_empty());
    for (id, name, tag) in [
        (ATTRS[0], "ОписаниеМетаданных", "S"),
        (ATTRS[1], "ДатаПоследнегоИспользования", "D"),
    ] {
        let d = ds.iter().find(|d| d.object_guid.as_str() == id).unwrap();
        assert_eq!(d.name, name);
        let ts = d.declared_types.as_ref().expect("измеренная декларация");
        assert_eq!(ts.len(), 1);
        assert_eq!(ts[0].tag, tag);
        assert!(ts[0].reference.is_none());
        assert!(ts[0].qualifiers.is_empty());
    }
}
#[test]
fn malformed_late_rows_and_local_owner_publish_no_type_prefix() {
    let base = acquired();
    let expected_owner = format!(
        "{:?}",
        (
            &owner(&base).catalog_attributes,
            &owner(&base).catalog_standard_attributes,
            &owner(&base).catalog_tabular_sections,
            &owner(&base).catalog_owners,
            &owner(&base).catalog_hierarchy_type,
            &owner(&base).catalog_default_presentation
        )
    );
    for mode in 0..37 {
        let mut ds = base.clone();
        for d in &mut ds {
            d.declared_types = None;
        }
        let mut v =
            super::super::parse_serialized(&super::super::inflate_raw_deflate(RAW).unwrap())
                .unwrap();
        match mode {
            0 => own(&mut v)[1] = Value::Atom("1".into()),
            1 => wrapper(own(&mut v).last_mut().unwrap())[0] = Value::Atom("4".into()),
            2 => {
                wrapper(own(&mut v).last_mut().unwrap()).pop();
            }
            3 => wrapper(own(&mut v).last_mut().unwrap())[2] = Value::Atom("1".into()),
            4 => wrapper(own(&mut v).last_mut().unwrap())[3] = Value::Atom("1".into()),
            5 => wrapper(own(&mut v).last_mut().unwrap())[4] = Value::Atom("0".into()),
            6 => wrapper(own(&mut v).last_mut().unwrap())[5] = Value::Atom("0".into()),
            7 => {
                vals(&mut wrapper(own(&mut v).last_mut().unwrap())[1])[0] = Value::Atom("26".into())
            }
            8 => {
                vals(&mut wrapper(own(&mut v).last_mut().unwrap())[1]).pop();
            }
            9 => typed(own(&mut v).last_mut().unwrap())[0] = Value::Atom("3".into()),
            10 => typed(own(&mut v).last_mut().unwrap())[2] = Value::Null,
            11 => {
                typed(own(&mut v).last_mut().unwrap())[2] =
                    Value::List(vec![Value::Atom("Pattern".into())])
            }
            12 => identity(own(&mut v).last_mut().unwrap())[0] = Value::Atom("1".into()),
            13 => {
                identity(own(&mut v).last_mut().unwrap()).pop();
            }
            14 => identity(own(&mut v).last_mut().unwrap())[4] = Value::Atom(String::new()),
            15 => identity(own(&mut v).last_mut().unwrap())[4] = Value::Null,
            16 => identity(own(&mut v).last_mut().unwrap())[4] = Value::List(vec![]),
            17 => {
                identity(own(&mut v).last_mut().unwrap())[4] = Value::String("неизмеренный".into())
            }
            18 => identity(own(&mut v).last_mut().unwrap())[5] = Value::Atom("1".into()),
            19 => identity(own(&mut v).last_mut().unwrap())[6] = Value::Atom("1".into()),
            20 => identity(own(&mut v).last_mut().unwrap())[3] = Value::Null,
            21 => {
                let id = identity(&mut own(&mut v)[2])[1].clone();
                identity(own(&mut v).last_mut().unwrap())[1] = id;
            }
            22 => {
                vals(&mut identity(own(&mut v).last_mut().unwrap())[1])[2] = Value::Atom(ID.into())
            }
            23 => ds.retain(|d| d.object_guid.as_str() != ATTRS[1]),
            24 => {
                let d = ds
                    .iter()
                    .find(|d| d.object_guid.as_str() == ATTRS[1])
                    .unwrap()
                    .clone();
                ds.push(d);
            }
            25 => owner_identity(&mut v)[4] = Value::String("неизмеренный".into()),
            26 => owner_identity(&mut v)[4] = Value::Null,
            27 => owner_identity(&mut v)[3] = Value::Null,
            28 => owner_identity(&mut v)[5] = Value::Atom("1".into()),
            29 => owner_identity(&mut v)[6] = Value::Atom("0".into()),
            30 => owner_identity(&mut v)[7] = Value::Atom(ID.into()),
            31 => owner_identity(&mut v)[8] = Value::Atom("3".into()),
            32 => owner_identity(&mut v)[9] = Value::Atom(ID.into()),
            33 => {
                owner_identity(&mut v).pop();
            }
            34 => {
                let d = owner(&ds).clone();
                ds.push(d);
            }
            35 => ds
                .iter_mut()
                .find(|d| d.object_guid.as_str() == ID)
                .unwrap()
                .catalog_attributes
                .as_mut()
                .unwrap()
                .swap(0, 1),
            _ => vals(&mut vals(&mut v)[1])[0] = Value::Atom("55".into()),
        }
        project(&v, &ID.parse().unwrap(), &mut ds);
        assert!(ds.iter().all(|d| d.declared_types.is_none()), "mode {mode}");
        if mode != 35 {
            assert_eq!(
                format!(
                    "{:?}",
                    (
                        &owner(&ds).catalog_attributes,
                        &owner(&ds).catalog_standard_attributes,
                        &owner(&ds).catalog_tabular_sections,
                        &owner(&ds).catalog_owners,
                        &owner(&ds).catalog_hierarchy_type,
                        &owner(&ds).catalog_default_presentation
                    )
                ),
                expected_owner,
                "mode {mode}"
            );
        }
    }
}
