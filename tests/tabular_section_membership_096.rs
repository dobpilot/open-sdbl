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
fn sections(value: &mut Value) -> &mut Vec<Value> {
    list(value)
        .iter_mut()
        .find_map(|v| match v {
            Value::List(items)
                if items.first()
                    == Some(&Value::Atom("21c53e09-8950-4b5e-a6a0-1054f1bbc274".into())) =>
            {
                Some(items)
            }
            _ => None,
        })
        .unwrap()
}
fn fields(section: &mut Value) -> &mut Vec<Value> {
    list(&mut list(section)[2])
}
fn event() -> Value {
    parse_serialized(&inflate_raw_deflate(EVENT_BYTES).unwrap()).unwrap()
}
fn acquired(value: &Value) -> Vec<open_sdbl::metadata::ConfigDescriptor> {
    parse_config_descriptors(EVENT, &stored_deflate(serialize(value).as_bytes())).unwrap()
}

#[test]
fn all_native_document_sections_match_exact_xml_membership() {
    let expected = include_str!("fixtures/runtime-metadata-096/membership-native.tsv")
        .lines()
        .filter(|line| line.starts_with("T\t"))
        .map(|line| {
            let cells = line.split('\t').collect::<Vec<_>>();
            (cells[1], cells)
        })
        .collect::<HashMap<_, _>>();
    let pack = include_bytes!("fixtures/unf/config.pack");
    let mut offset = 0;
    let (mut parts, mut attrs) = (0, 0);
    while offset < pack.len() {
        let end = offset + pack[offset..].iter().position(|b| *b == b'\n').unwrap();
        let (id, n) = std::str::from_utf8(&pack[offset..end])
            .unwrap()
            .split_once('\t')
            .unwrap();
        let n: usize = n.parse().unwrap();
        offset = end + 1;
        let descriptors = parse_config_descriptors(id, &pack[offset..offset + n]).unwrap();
        for d in &descriptors {
            if let Some(cells) = expected.get(d.object_guid.as_str()) {
                let ids = cells[3]
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.parse::<Guid>().unwrap())
                    .collect::<Vec<_>>();
                assert_eq!(
                    d.tabular_section_attributes,
                    Some(ids.clone()),
                    "{}",
                    cells[2]
                );
                for id in &ids {
                    let field = descriptors.iter().find(|d| &d.object_guid == id).unwrap();
                    assert!(
                        field.declared_types.is_some(),
                        "{}.{}",
                        cells[2],
                        field.name
                    );
                    assert!(
                        field.fill_from_filling_value.is_some(),
                        "{}.{}",
                        cells[2],
                        field.name
                    );
                    assert!(field.filling_value.is_some(), "{}.{}", cells[2], field.name);
                }
                parts += 1;
                attrs += ids.len();
            }
        }
        offset += n;
    }
    assert_eq!((parts, attrs), (522, 4519));
}

#[test]
fn separate_sections_and_empty_membership_do_not_leak_neighbors() {
    let mut v = event();
    let sections = sections(&mut v);
    sections.truncate(5);
    sections[1] = Value::Atom("3".into());
    let empty = fields(&mut sections[4]);
    empty.truncate(2);
    empty[1] = Value::Atom("0".into());
    let ds = acquired(&v);
    let names = [
        "ДокументыОснования",
        "УдалитьУчастники",
        "ДополнительныеРеквизиты",
    ];
    let members = names.map(|name| {
        ds.iter()
            .find(|d| d.name == name)
            .unwrap()
            .tabular_section_attributes
            .clone()
            .unwrap()
    });
    assert_eq!(members[0].len(), 1);
    assert!(!members[1].is_empty());
    assert!(members[2].is_empty());
    assert!(members[0].iter().all(|id| !members[1].contains(id)));
}

#[test]
fn malformed_membership_is_unknown_without_damaging_other_sections() {
    for mode in 0..4 {
        let mut v = event();
        let own = fields(&mut sections(&mut v)[2]);
        match mode {
            0 => own[1] = Value::Atom("2".into()),
            1 => {
                own[0] = Value::Atom("00000000-0000-0000-0000-000000000000".into());
            }
            2 => {
                let duplicate = own[2].clone();
                own.push(duplicate);
                own[1] = Value::Atom("2".into());
            }
            _ => {
                own.truncate(2);
            }
        }
        let ds = acquired(&v);
        assert!(
            ds.iter()
                .find(|d| d.name == "ДокументыОснования")
                .unwrap()
                .tabular_section_attributes
                .is_none()
        );
        assert!(
            ds.iter()
                .find(|d| d.name == "ДополнительныеРеквизиты")
                .unwrap()
                .tabular_section_attributes
                .is_some()
        );
    }
}

#[test]
fn unknown_member_declaration_remains_unknown_with_known_membership() {
    let mut v = event();
    let own = fields(&mut sections(&mut v)[2]);
    let body = &mut list(&mut list(&mut own[2])[0])[1];
    let declaration = &mut list(&mut list(body)[1])[2];
    *declaration = Value::Atom("unknown".into());
    let ds = acquired(&v);
    let section = ds.iter().find(|d| d.name == "ДокументыОснования").unwrap();
    let id = &section.tabular_section_attributes.as_ref().unwrap()[0];
    assert!(
        ds.iter()
            .find(|d| &d.object_guid == id)
            .unwrap()
            .declared_types
            .is_none()
    );
}
