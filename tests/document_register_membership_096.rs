mod support;

use open_sdbl::metadata::{
    Guid, Value, inflate_raw_deflate, parse_config_descriptors, parse_serialized,
};
use std::collections::HashMap;
use support::stored_deflate;

const EVENT: &str = "571e1bd9-0f64-4a05-a276-dba4fe242ad0";
const EVENT_BYTES: &[u8] = include_bytes!("fixtures/runtime-metadata-096/event.deflate");
fn list(value: &mut Value) -> &mut Vec<Value> {
    let Value::List(items) = value else {
        panic!("ожидалась коллекция")
    };
    items
}
fn serialize(value: &Value) -> String {
    match value {
        Value::List(items) => format!(
            "{{{}}}",
            items.iter().map(serialize).collect::<Vec<_>>().join(",")
        ),
        Value::String(s) => format!("\"{}\"", s.replace('"', "\"\"")),
        Value::Atom(s) => s.clone(),
        Value::Null => String::new(),
    }
}
fn event() -> Value {
    parse_serialized(&inflate_raw_deflate(EVENT_BYTES).unwrap()).unwrap()
}
fn acquired(value: &Value) -> Option<Vec<Guid>> {
    parse_config_descriptors(EVENT, &stored_deflate(serialize(value).as_bytes()))
        .unwrap()
        .into_iter()
        .find(|d| d.object_guid.as_str() == EVENT)
        .unwrap()
        .document_registers
}
fn registers(value: &mut Value) -> &mut Vec<Value> {
    list(&mut list(&mut list(value)[1])[24])
}

#[test]
fn all_native_document_registers_match_exact_xml_order_and_identity() {
    let expected = include_str!("fixtures/runtime-metadata-096/membership-native.tsv")
        .lines()
        .filter(|line| line.starts_with("D\t"))
        .map(|line| {
            let cells = line.split('\t').collect::<Vec<_>>();
            (cells[1], cells)
        })
        .collect::<HashMap<_, _>>();
    let pack = include_bytes!("fixtures/unf/config.pack");
    let mut offset = 0;
    let (mut docs, mut refs, mut empty) = (0, 0, 0);
    while offset < pack.len() {
        let end = offset + pack[offset..].iter().position(|b| *b == b'\n').unwrap();
        let (id, n) = std::str::from_utf8(&pack[offset..end])
            .unwrap()
            .split_once('\t')
            .unwrap();
        let n: usize = n.parse().unwrap();
        offset = end + 1;
        if let Some(cells) = expected.get(id) {
            let ds = parse_config_descriptors(id, &pack[offset..offset + n]).unwrap();
            let owner = ds.iter().find(|d| d.object_guid.as_str() == id).unwrap();
            let ids = cells[3]
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.parse::<Guid>().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(owner.document_registers, Some(ids.clone()), "{}", cells[2]);
            if cells[2] == "ЗаказПокупателя" {
                assert_eq!(ids.len(), 51);
                let classes = cells[4]
                    .split(',')
                    .collect::<std::collections::HashSet<_>>();
                assert!(classes.contains("InformationRegister"));
                assert!(classes.contains("AccumulationRegister"));
            }
            if cells[2] == "Событие" {
                assert_eq!(
                    ids,
                    vec![
                        "c34d5785-6f04-45c7-9555-088544ddc489"
                            .parse::<Guid>()
                            .unwrap()
                    ]
                );
            }
            empty += usize::from(ids.is_empty());
            refs += ids.len();
            docs += 1;
        }
        offset += n;
    }
    assert_eq!((docs, refs), (125, 1355));
    assert!(empty > 0);
}

#[test]
fn malformed_or_unknown_register_collections_never_return_partial_success() {
    for mode in 0..7 {
        let mut v = event();
        let own = registers(&mut v);
        match mode {
            0 => own[1] = Value::Atom("2".into()),
            1 => {
                own.truncate(2);
            }
            2 => {
                own[0] = Value::Atom("unknown".into());
            }
            3 => {
                let entry = own[2].clone();
                own.push(entry);
                own[1] = Value::Atom("2".into());
            }
            4 => {
                list(&mut list(&mut own[2])[2])[1] =
                    Value::Atom("00000000-0000-0000-0000-000000000000".into());
            }
            5 => {
                list(&mut own[2])[1] = Value::Atom("unknown".into());
            }
            _ => {
                list(&mut list(&mut v)[1]).pop();
            }
        }
        assert!(acquired(&v).is_none(), "mode={mode}");
    }
}

#[test]
fn unrelated_guids_do_not_change_known_membership() {
    let mut v = event();
    let expected = acquired(&v).unwrap();
    list(&mut list(&mut v)[1])[16] = Value::Atom("00000000-0000-0000-0000-000000000123".into());
    assert_eq!(acquired(&v), Some(expected));
    let own = registers(&mut v);
    own.truncate(2);
    own[1] = Value::Atom("0".into());
    assert_eq!(acquired(&v), Some(vec![]));
}
