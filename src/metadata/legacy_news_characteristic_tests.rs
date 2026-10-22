use super::*;

const ID: &str = "60c8bc2e-853b-4622-8dbf-7628931c5c59";
const RAW: &[u8] = include_bytes!(
    "../../tests/fixtures/buh-news-characteristic/60c8bc2e-853b-4622-8dbf-7628931c5c59.deflate"
);
const XML: &str =
    include_str!("../../tests/fixtures/buh-news-characteristic/ChartOfCharacteristicTypes.xml");
const COLLECTION: &str = "31182525-9346-4595-81f8-6f91a72ebe06";

fn acquired() -> Vec<ConfigDescriptor> {
    super::super::parse_config_descriptors(ID, RAW).unwrap()
}
fn source() -> Value {
    super::super::parse_serialized(&super::super::inflate_raw_deflate(RAW).unwrap()).unwrap()
}
fn owner(ds: &[ConfigDescriptor]) -> &ConfigDescriptor {
    ds.iter().find(|d| d.object_guid.as_str() == ID).unwrap()
}
fn values(v: &mut Value) -> &mut Vec<Value> {
    match v {
        Value::List(v) => v,
        _ => panic!("ожидается измеренная коллекция"),
    }
}
fn rows(v: &mut Value) -> &mut Vec<Value> {
    values(v)
        .iter_mut()
        .find_map(|v| match v {
            Value::List(rows) if atom(&rows[0]) == Some(COLLECTION) => Some(rows),
            _ => None,
        })
        .unwrap()
}
fn typed(row: &mut Value) -> &mut Vec<Value> {
    values(&mut values(&mut values(&mut values(row)[0])[1])[1])
}
fn mutated(edit: impl FnOnce(&mut Value, &mut Vec<ConfigDescriptor>)) -> Vec<ConfigDescriptor> {
    let mut value = source();
    let mut ds = acquired();
    for d in &mut ds {
        d.characteristic_settings = None;
        d.characteristic_attributes = None;
        d.declared_types = None;
    }
    edit(&mut value, &mut ds);
    project(&value, &ID.parse().unwrap(), &mut ds);
    ds
}

#[test]
fn native_news_characteristic_matches_all_xml_declarations_settings_and_generated_types() {
    let source = source();
    let class = at(&source, &[1]).and_then(list).unwrap();
    assert_eq!(class.len(), 56);
    assert_eq!(atom(&class[0]), Some("32"));
    assert_eq!(at(&source, &[1, 13, 1]).and_then(list).unwrap().len(), 9);
    for (position, expected) in [
        (1, "70a511ca-0fab-47ab-b34d-c3e215318f70"),
        (2, "a780a3ba-5088-48ee-b4d5-01bf762dd6b7"),
        (3, "a55336a2-d56f-4c88-bf29-ed5e6ef4c7de"),
        (4, "cacd0eb6-268a-4778-b661-0056de81e20f"),
    ] {
        assert!(XML.contains(expected));
        assert_eq!(atom(&class[position]), Some(expected));
    }
    let ds = acquired();
    let own = owner(&ds);
    assert_eq!(own.name, "КатегорииНовостей");
    assert_eq!(
        own.object_reference_type.as_ref().unwrap().as_str(),
        "a55336a2-d56f-4c88-bf29-ed5e6ef4c7de"
    );
    let settings = own.characteristic_settings.as_ref().unwrap();
    assert!(XML.contains("<CodeLength>20</CodeLength>"));
    assert!(XML.contains("<DescriptionLength>50</DescriptionLength>"));
    assert_eq!(
        (settings.code_length, settings.description_length),
        (20, 50)
    );
    let expected_values = [
        ("B", "", vec![]),
        ("S", "", vec!["100", "1"]),
        ("#", "a1f87530-5261-4cac-aad9-a636c0e39e38", vec![]),
        ("D", "", vec![]),
        ("N", "", vec!["28", "4", "0"]),
    ];
    assert_eq!(settings.value_types.len(), expected_values.len());
    for (ty, (tag, reference, qualifiers)) in settings.value_types.iter().zip(expected_values) {
        assert_eq!(ty.tag, tag);
        assert_eq!(
            ty.reference.as_ref().map(Guid::as_str).unwrap_or(""),
            reference
        );
        assert_eq!(
            ty.qualifiers,
            qualifiers
                .into_iter()
                .map(|v| Value::Atom(v.into()))
                .collect::<Vec<_>>()
        );
    }
    let catalog_xml = include_str!("../../tests/fixtures/buh-news-characteristic/ValueCatalog.xml");
    assert!(catalog_xml.contains("<xr:TypeId>a1f87530-5261-4cac-aad9-a636c0e39e38</xr:TypeId>"));
    let enum_xml = include_str!("../../tests/fixtures/buh-news-characteristic/TypeEnumeration.xml");
    assert!(enum_xml.contains("<xr:TypeId>987f29d7-195e-43b5-8325-da3afb6bd224</xr:TypeId>"));
    let ids = own.characteristic_attributes.as_ref().unwrap();
    let expected = include_str!("../../tests/fixtures/buh-news-characteristic/xml-attributes.tsv")
        .lines()
        .skip(1)
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), 5);
    assert_eq!(ids.len(), expected.len());
    for (id, row) in ids.iter().zip(expected) {
        let columns = row.split('\t').collect::<Vec<_>>();
        assert_eq!(id.as_str(), columns[0]);
        let d = ds.iter().find(|d| &d.object_guid == id).unwrap();
        assert_eq!(d.name, columns[1]);
        let types = d.declared_types.as_ref().unwrap();
        assert_eq!(types.len(), 1);
        assert!(types[0].qualifiers.is_empty());
        if columns[2] == "xs:boolean" {
            assert_eq!(types[0].tag, "B");
            assert!(types[0].reference.is_none());
        } else {
            assert_eq!(columns[2], "cfg:EnumRef.ТипыЗначенийКатегорийНовостей");
            assert_eq!(types[0].tag, "#");
            assert_eq!(
                types[0].reference.as_ref().unwrap().as_str(),
                "987f29d7-195e-43b5-8325-da3afb6bd224"
            );
        }
    }
}

#[test]
fn malformed_news_owners_do_not_enable_the_legacy_profile() {
    for mode in 0..7 {
        let ds = mutated(|v, _| {
            let class = values(&mut values(v)[1]);
            match mode {
                0 => class.push(Value::Atom("0".into())),
                1 => class[0] = Value::Atom("34".into()),
                2 => class[1] = Value::String("not-a-guid".into()),
                3 => values(&mut class[13])[1] = Value::Null,
                4 => values(&mut values(&mut class[13])[1])[0] = Value::Atom("2".into()),
                5 => class[21] = Value::String("20".into()),
                6 => class[18] = Value::Null,
                _ => unreachable!(),
            }
        });
        assert!(owner(&ds).characteristic_settings.is_none(), "mode {mode}");
        assert!(
            owner(&ds).characteristic_attributes.is_none(),
            "mode {mode}"
        );
        assert!(ds.iter().all(|d| d.declared_types.is_none()), "mode {mode}");
    }
}

#[test]
fn malformed_news_members_do_not_publish_partial_declarations_or_membership() {
    for mode in 0..11 {
        let ds = mutated(|v, ds| {
            let rows = rows(v);
            let last = rows.len() - 1;
            match mode {
                0 => rows[1] = Value::Atom("4".into()),
                1 => {
                    rows.pop();
                }
                2 => rows[last] = rows[2].clone(),
                3 => values(&mut values(&mut rows[last])[0])[0] = Value::Atom("2".into()),
                4 => {
                    values(&mut values(&mut rows[last])[0]).pop();
                }
                5 => values(&mut typed(&mut rows[last])[1])[2] = Value::Atom("0".into()),
                6 => typed(&mut rows[last])[2] = Value::Null,
                7 => {
                    typed(&mut rows[last])[2] = Value::List(vec![
                        Value::String("Pattern".into()),
                        Value::List(vec![
                            Value::String("#".into()),
                            Value::String("not-a-guid".into()),
                        ]),
                    ])
                }
                8 => ds.retain(|d| d.name != "ЗагруженоССервера"),
                9 => ds.push(
                    ds.iter()
                        .find(|d| d.name == "ЗагруженоССервера")
                        .unwrap()
                        .clone(),
                ),
                10 => values(&mut typed(&mut rows[last])[1])[4] = Value::Null,
                _ => unreachable!(),
            }
        });
        assert!(owner(&ds).characteristic_settings.is_some(), "mode {mode}");
        assert!(
            owner(&ds).characteristic_attributes.is_none(),
            "mode {mode}"
        );
        assert!(ds.iter().all(|d| d.declared_types.is_none()), "mode {mode}");
    }
}
