mod support;

use open_sdbl::metadata::{
    DocumentNumberPeriodicity, DocumentNumberingSettings, Guid, Value, inflate_raw_deflate,
    parse_config_descriptors, parse_serialized,
};
use std::collections::HashMap;
use support::stored_deflate;

const EVENT: &str = "571e1bd9-0f64-4a05-a276-dba4fe242ad0";
fn periodicity(name: &str) -> DocumentNumberPeriodicity {
    match name {
        "Nonperiodical" => DocumentNumberPeriodicity::Nonperiodical,
        "Year" => DocumentNumberPeriodicity::Year,
        "Quarter" => DocumentNumberPeriodicity::Quarter,
        "Month" => DocumentNumberPeriodicity::Month,
        "Day" => DocumentNumberPeriodicity::Day,
        _ => panic!("неизвестный нативный эталон"),
    }
}
fn expected(cells: &[&str]) -> DocumentNumberingSettings {
    DocumentNumberingSettings {
        periodicity: periodicity(cells[2]),
        numerator: (!cells[3].is_empty()).then(|| cells[3].parse::<Guid>().unwrap()),
    }
}

#[test]
fn all_native_unf_documents_match_periodicity_and_numerator_object_identity() {
    let oracle = include_str!("fixtures/document-numbering-096/unf-native.tsv")
        .lines()
        .map(|line| {
            let cells = line.split('\t').collect::<Vec<_>>();
            (cells[0], cells)
        })
        .collect::<HashMap<_, _>>();
    let pack = include_bytes!("fixtures/unf/config.pack");
    let mut offset = 0;
    let (mut docs, mut assigned) = (0, 0);
    while offset < pack.len() {
        let end = offset + pack[offset..].iter().position(|b| *b == b'\n').unwrap();
        let (id, n) = std::str::from_utf8(&pack[offset..end])
            .unwrap()
            .split_once('\t')
            .unwrap();
        let n: usize = n.parse().unwrap();
        offset = end + 1;
        if let Some(cells) = oracle.get(id) {
            let ds = parse_config_descriptors(id, &pack[offset..offset + n]).unwrap();
            let owner = ds.iter().find(|d| d.object_guid.as_str() == id).unwrap();
            let settings = expected(cells);
            assigned += usize::from(settings.numerator.is_some());
            assert_eq!(owner.document_numbering, Some(settings), "{}", cells[1]);
            assert!(
                ds.iter()
                    .filter(|d| d.object_guid.as_str() != id)
                    .all(|d| d.document_numbering.is_none())
            );
            if cells[1] == "ЗаказПокупателя" {
                assert_eq!(owner.document_number.as_ref().unwrap().length, 11);
                assert_eq!(
                    owner.document_numbering.as_ref().unwrap().periodicity,
                    DocumentNumberPeriodicity::Year
                );
                assert!(
                    owner
                        .document_numbering
                        .as_ref()
                        .unwrap()
                        .numerator
                        .is_none()
                );
            }
            docs += 1;
        }
        offset += n;
    }
    assert_eq!((docs, assigned), (125, 2));
}

#[test]
fn native_minimum_covers_every_periodicity_and_two_shared_numerator_links() {
    let cases: [(&str, &[u8]); 7] = [
        (
            "PeriodNone",
            include_bytes!("fixtures/document-numbering-096/PeriodNone.deflate"),
        ),
        (
            "PeriodYear",
            include_bytes!("fixtures/document-numbering-096/PeriodYear.deflate"),
        ),
        (
            "PeriodQuarter",
            include_bytes!("fixtures/document-numbering-096/PeriodQuarter.deflate"),
        ),
        (
            "PeriodMonth",
            include_bytes!("fixtures/document-numbering-096/PeriodMonth.deflate"),
        ),
        (
            "PeriodDay",
            include_bytes!("fixtures/document-numbering-096/PeriodDay.deflate"),
        ),
        (
            "SharedA",
            include_bytes!("fixtures/document-numbering-096/SharedA.deflate"),
        ),
        (
            "SharedB",
            include_bytes!("fixtures/document-numbering-096/SharedB.deflate"),
        ),
    ];
    let oracle = include_str!("fixtures/document-numbering-096/minimal-native.tsv")
        .lines()
        .map(|line| {
            let cells = line.split('\t').collect::<Vec<_>>();
            (cells[1], cells)
        })
        .collect::<HashMap<_, _>>();
    let mut shared = Vec::new();
    for (name, bytes) in cases {
        let cells = &oracle[name];
        let ds = parse_config_descriptors(cells[0], bytes).unwrap();
        let owner = ds
            .iter()
            .find(|d| d.object_guid.as_str() == cells[0])
            .unwrap();
        assert_eq!(owner.document_numbering, Some(expected(cells)), "{name}");
        if name.starts_with("Shared") {
            let numerator = owner
                .document_numbering
                .as_ref()
                .unwrap()
                .numerator
                .as_ref()
                .unwrap();
            assert_ne!(numerator, &owner.object_guid);
            assert_eq!(numerator.as_str(), "81395a18-e649-5291-be0d-c702ef12e04b");
            shared.push(numerator.clone());
        }
    }
    assert_eq!(shared.len(), 2);
    assert_eq!(shared[0], shared[1]);
}

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

#[test]
fn unmeasured_and_malformed_numbering_is_unknown_without_fabricated_absence() {
    let original = parse_serialized(
        &inflate_raw_deflate(include_bytes!(
            "fixtures/runtime-metadata-096/event.deflate"
        ))
        .unwrap(),
    )
    .unwrap();
    for mode in 0..6 {
        let mut v = original.clone();
        let class = list(&mut list(&mut v)[1]);
        match mode {
            0 => class[13] = Value::Atom("5".into()),
            1 => class[13] = Value::List(Vec::new()),
            2 => class[10] = Value::Atom("malformed-guid".into()),
            3 => class[10] = Value::List(Vec::new()),
            4 => {
                class.pop();
            }
            _ => class.push(Value::Atom("0".into())),
        }
        let ds =
            parse_config_descriptors(EVENT, &stored_deflate(serialize(&v).as_bytes())).unwrap();
        let owner = ds.iter().find(|d| d.object_guid.as_str() == EVENT).unwrap();
        assert!(owner.document_numbering.is_none(), "mode={mode}");
        if mode < 4 {
            assert!(
                owner.document_number.is_some(),
                "старые настройки не зависят от новых"
            );
        }
    }
}
