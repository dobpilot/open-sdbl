use super::*;

const ID: &str = "642117be-dba4-4c6c-b5e7-c2821543a830";
const RAW: &[u8] = include_bytes!(
    "../../tests/fixtures/buh-sales-book-document/642117be-dba4-4c6c-b5e7-c2821543a830.deflate"
);
const EXPECTED: &str =
    include_str!("../../tests/fixtures/buh-sales-book-document/xml-declarations.tsv");
const COLLECTION: &str = "45e46cbc-3e24-4165-8b7b-cc98a6f80211";
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
        d.document_attributes = None;
        d.declared_types = None;
    }
    edit(&mut value, &mut ds);
    project(&value, &ID.parse().unwrap(), &mut ds);
    ds
}
#[test]
fn native_sales_book_document_preserves_all_seven_xml_declarations_and_order() {
    let ds = acquired();
    let own = owner(&ds);
    assert_eq!(own.name, "ФормированиеЗаписейКнигиПродаж");
    let ids = own.document_attributes.as_ref().unwrap();
    let expected = EXPECTED.lines().collect::<Vec<_>>();
    assert_eq!(ids.len(), 7);
    assert_eq!(ids.len(), expected.len());
    for (id, row) in ids.iter().zip(expected) {
        let c = row.split('\t').collect::<Vec<_>>();
        assert_eq!(id.as_str(), c[0]);
        let d = ds.iter().find(|d| &d.object_guid == id).unwrap();
        assert_eq!(d.name, c[1]);
        let types = d.declared_types.as_ref().unwrap();
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].tag, c[2]);
        assert_eq!(
            types[0].reference.as_ref().map(Guid::as_str).unwrap_or(""),
            c[3]
        );
        assert_eq!(
            types[0]
                .qualifiers
                .iter()
                .map(|v| v.as_string().or_else(|| atom(v)).unwrap())
                .collect::<Vec<_>>()
                .join(","),
            c[4]
        );
    }
    let xml = include_str!("../../tests/fixtures/buh-sales-book-document/Document.xml");
    assert!(xml.contains("<xr:TypeId>b1ab630a-60da-4546-9d87-d9af8b9c7814</xr:TypeId>"));
    assert!(xml.contains("<xr:TypeId>99eb9209-179e-4acd-9301-27103c6a4765</xr:TypeId>"));
    assert_eq!(
        own.object_reference_type.as_ref().unwrap().as_str(),
        "b1ab630a-60da-4546-9d87-d9af8b9c7814"
    );
    assert!(
        include_str!("../../tests/fixtures/buh-sales-book-document/Организации.xml")
            .contains("<xr:TypeId>c92a1124-c41e-42a4-bda7-3bbac9c6039e</xr:TypeId>")
    );
    assert!(
        include_str!("../../tests/fixtures/buh-sales-book-document/Пользователи.xml")
            .contains("<xr:TypeId>bb356e79-e8e6-4912-8561-ad29cca2604f</xr:TypeId>")
    );
    // Позиции современного документа не определяют настройки старого класса.
    assert!(own.document_numbering.is_none());
    assert!(own.document_number.is_none());
    assert!(own.document_registers.is_none());
    assert_eq!(own.document_tabular_sections.as_ref().unwrap().len(), 6);
}
#[test]
fn malformed_legacy_document_owner_does_not_enable_the_profile() {
    for mode in 0..6 {
        let ds = mutated(|v, _| {
            let class = values(&mut values(v)[1]);
            match mode {
                0 => class.push(Value::Atom("0".into())),
                1 => class[0] = Value::Atom("37".into()),
                2 => class[1] = Value::Null,
                3 => values(&mut class[9])[1] = Value::Null,
                4 => values(&mut values(&mut class[9])[1])[0] = Value::Atom("2".into()),
                5 => values(&mut values(&mut class[9])[1])[2] = Value::Atom("0".into()),
                _ => unreachable!(),
            }
        });
        assert!(owner(&ds).document_attributes.is_none(), "mode {mode}");
        assert!(ds.iter().all(|d| d.declared_types.is_none()), "mode {mode}");
    }
}
#[test]
fn malformed_legacy_document_members_do_not_publish_a_prefix() {
    for mode in 0..11 {
        let ds = mutated(|v, ds| {
            let rows = rows(v);
            let last = rows.len() - 1;
            match mode {
                0 => rows[1] = Value::Atom("6".into()),
                1 => {
                    rows.pop();
                }
                2 => rows[last] = rows[2].clone(),
                3 => values(&mut values(&mut rows[last])[0])[0] = Value::Atom("5".into()),
                4 => {
                    values(&mut values(&mut rows[last])[0]).pop();
                }
                5 => typed(&mut rows[last])[1] = Value::Null,
                6 => typed(&mut rows[last])[2] = Value::Null,
                7 => {
                    typed(&mut rows[last])[2] = Value::List(vec![
                        Value::String("Pattern".into()),
                        Value::List(vec![Value::String("unsupported".into())]),
                    ])
                }
                8 => ds.retain(|d| d.name != "ДокументСозданВПомощнике"),
                9 => ds.push(
                    ds.iter()
                        .find(|d| d.name == "ДокументСозданВПомощнике")
                        .unwrap()
                        .clone(),
                ),
                10 => values(&mut typed(&mut rows[last])[1])[0] = Value::Atom("3".into()),
                _ => unreachable!(),
            }
        });
        assert!(owner(&ds).document_attributes.is_none(), "mode {mode}");
        assert!(ds.iter().all(|d| d.declared_types.is_none()), "mode {mode}");
    }
}
