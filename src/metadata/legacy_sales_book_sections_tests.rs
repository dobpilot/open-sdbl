use super::*;
const ID: &str = "642117be-dba4-4c6c-b5e7-c2821543a830";
const RAW: &[u8] = include_bytes!(
    "../../tests/fixtures/buh-sales-book-document/642117be-dba4-4c6c-b5e7-c2821543a830.deflate"
);
const PARTS: &str = include_str!("../../tests/fixtures/buh-sales-book-sections/xml-sections.tsv");
const FIELDS: &str = include_str!("../../tests/fixtures/buh-sales-book-sections/xml-fields.tsv");
const TYPES: &str =
    include_str!("../../tests/fixtures/buh-sales-book-sections/xml-fields-types.tsv");
fn acquired() -> Vec<ConfigDescriptor> {
    super::super::parse_config_descriptors(ID, RAW).unwrap()
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
fn collection_rows<'a>(v: &'a mut Value, id: &str) -> &'a mut Vec<Value> {
    values(v)
        .iter_mut()
        .find_map(|v| match v {
            Value::List(rows) if atom(&rows[0]) == Some(id) => Some(rows),
            _ => None,
        })
        .unwrap()
}
fn sections(v: &mut Value) -> &mut Vec<Value> {
    collection_rows(v, "21c53e09-8950-4b5e-a6a0-1054f1bbc274")
}
fn fields(v: &mut Value) -> &mut Vec<Value> {
    collection_rows(v, "888744e1-b616-11d4-9436-004095e12fc7")
}
fn typed(row: &mut Value) -> &mut Vec<Value> {
    values(&mut values(&mut values(&mut values(row)[0])[1])[1])
}
fn mutated(edit: impl FnOnce(&mut Value, &mut Vec<ConfigDescriptor>)) -> Vec<ConfigDescriptor> {
    let text = super::super::inflate_raw_deflate(RAW).unwrap();
    let mut tree = super::super::parse_serialized(&text).unwrap();
    let mut ds = acquired();
    let own_ids = owner(&ds).document_attributes.clone().unwrap();
    for d in &mut ds {
        d.document_tabular_sections = None;
        if !own_ids.contains(&d.object_guid) {
            d.declared_types = None;
            d.tabular_section_attributes = None;
        }
    }
    edit(&mut tree, &mut ds);
    project(&tree, &ID.parse().unwrap(), &mut ds);
    ds
}
fn assert_no_section_prefix_and_own_attributes_retained(ds: &[ConfigDescriptor], mode: usize) {
    let own = owner(ds);
    assert!(own.document_tabular_sections.is_none(), "mode {mode}");
    let own_ids = own.document_attributes.as_ref().unwrap();
    assert_eq!(own_ids.len(), 7);
    for d in ds {
        if own_ids.contains(&d.object_guid) {
            assert!(d.declared_types.is_some(), "mode {mode}");
        } else {
            assert!(d.declared_types.is_none(), "mode {mode}: {}", d.name);
            assert!(
                d.tabular_section_attributes.is_none(),
                "mode {mode}: {}",
                d.name
            );
        }
    }
}
#[test]
fn native_sales_book_sections_match_all_six_sections_68_fields_and_201_qualified_types() {
    let ds = acquired();
    let own = owner(&ds);
    assert_eq!(own.document_attributes.as_ref().unwrap().len(), 7);
    let ids = own.document_tabular_sections.as_ref().unwrap();
    let parts = PARTS
        .lines()
        .skip(1)
        .map(|s| s.split('\t').collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let fields = FIELDS
        .lines()
        .skip(1)
        .map(|s| s.split('\t').collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let types = TYPES
        .lines()
        .skip(1)
        .map(|s| s.split('\t').collect::<Vec<_>>())
        .collect::<Vec<_>>();
    assert_eq!(parts.len(), 6);
    assert_eq!(fields.len(), 68);
    assert_eq!(types.len(), 201);
    assert_eq!(ids.len(), 6);
    let mut field_count = 0;
    let mut type_count = 0;
    for (id, expected) in ids.iter().zip(parts) {
        assert_eq!(id.as_str(), expected[1]);
        let section = ds.iter().find(|d| &d.object_guid == id).unwrap();
        assert_eq!(section.name, expected[2]);
        let actual = section.tabular_section_attributes.as_ref().unwrap();
        let expected_fields = fields
            .iter()
            .filter(|r| r[0] == id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(actual.len(), expected[3].parse::<usize>().unwrap());
        assert_eq!(actual.len(), expected_fields.len());
        for (field_id, row) in actual.iter().zip(expected_fields) {
            assert_eq!(field_id.as_str(), row[2]);
            let field = ds.iter().find(|d| &d.object_guid == field_id).unwrap();
            assert_eq!(field.name, row[3]);
            let actual_types = field.declared_types.as_ref().unwrap();
            let expected_types = types
                .iter()
                .filter(|r| r[2] == field_id.as_str())
                .collect::<Vec<_>>();
            assert_eq!(actual_types.len(), expected_types.len());
            for (i, (ty, expected)) in actual_types.iter().zip(expected_types).enumerate() {
                assert_eq!(expected[4], i.to_string());
                assert_eq!(ty.tag, expected[5]);
                assert_eq!(
                    ty.reference.as_ref().map(Guid::as_str).unwrap_or(""),
                    expected[6]
                );
                assert_eq!(
                    ty.qualifiers
                        .iter()
                        .map(|v| v.as_string().or_else(|| atom(v)).unwrap())
                        .collect::<Vec<_>>()
                        .join(","),
                    expected[7]
                );
                type_count += 1;
            }
            field_count += 1;
        }
    }
    assert_eq!((field_count, type_count), (68, 201));
    assert!(own.document_numbering.is_none());
    assert!(own.document_number.is_none());
    assert!(own.document_registers.is_none());
}
#[test]
fn malformed_last_section_never_publishes_section_or_field_prefixes() {
    for mode in 0..10 {
        let ds = mutated(|v, ds| {
            let rows = sections(v);
            let last = rows.len() - 1;
            match mode {
                0 => rows[1] = Value::Atom("5".into()),
                1 => {
                    rows.pop();
                }
                2 => rows[last] = rows[2].clone(),
                3 => values(&mut values(&mut rows[last])[0])[0] = Value::Atom("0".into()),
                4 => {
                    values(&mut values(&mut values(&mut rows[last])[0])[1])[0] =
                        Value::Atom("10".into())
                }
                5 => values(&mut values(&mut values(&mut rows[last])[0])[1])[1] = Value::Null,
                6 => {
                    let body = values(&mut values(&mut values(&mut rows[last])[0])[1]);
                    values(&mut body[5])[1] = Value::Null;
                }
                7 => {
                    ds.retain(|d| d.object_guid.as_str() != "c306f18b-db4e-4b79-a64e-e30bfabdc04b")
                }
                8 => ds.push(
                    ds.iter()
                        .find(|d| d.object_guid.as_str() == "c306f18b-db4e-4b79-a64e-e30bfabdc04b")
                        .unwrap()
                        .clone(),
                ),
                9 => values(&mut rows[last])[1] = Value::Atom("0".into()),
                _ => unreachable!(),
            }
        });
        assert_no_section_prefix_and_own_attributes_retained(&ds, mode);
    }
}
#[test]
fn malformed_last_section_field_never_publishes_any_of_68_declarations() {
    for mode in 0..11 {
        let ds = mutated(|v, ds| {
            let rows = sections(v);
            let last = rows.len() - 1;
            let fields = fields(&mut rows[last]);
            let last = fields.len() - 1;
            let last_id = TYPES.lines().last().unwrap().split('\t').nth(2).unwrap();
            match mode {
                0 => fields[1] = Value::Atom("9".into()),
                1 => {
                    fields.pop();
                }
                2 => fields[last] = fields[2].clone(),
                3 => values(&mut values(&mut fields[last])[0])[0] = Value::Atom("8".into()),
                4 => typed(&mut fields[last])[1] = Value::Null,
                5 => typed(&mut fields[last])[2] = Value::Null,
                6 => {
                    typed(&mut fields[last])[2] = Value::List(vec![
                        Value::String("Pattern".into()),
                        Value::List(vec![Value::String("unsupported".into())]),
                    ])
                }
                7 => ds.retain(|d| d.object_guid.as_str() != last_id),
                8 => ds.push(
                    ds.iter()
                        .find(|d| d.object_guid.as_str() == last_id)
                        .unwrap()
                        .clone(),
                ),
                9 => values(&mut typed(&mut fields[last])[1])[0] = Value::Atom("2".into()),
                10 => {
                    values(&mut values(&mut fields[last])[0]).pop();
                }
                _ => unreachable!(),
            }
        });
        assert_no_section_prefix_and_own_attributes_retained(&ds, mode);
    }
}

#[test]
fn section_identities_cannot_alias_the_owner_or_its_existing_own_attributes() {
    let baseline = acquired();
    let own_ids = owner(&baseline).document_attributes.as_ref().unwrap();
    for mode in 0..4 {
        let target = if mode % 2 == 0 {
            "6c3542e3-d4fc-44e8-a0d1-6303bd31a7cb"
        } else {
            ID
        };
        let ds = mutated(|v, _| {
            let rows = sections(v);
            let last = rows.len() - 1;
            if mode < 2 {
                let fields = fields(&mut rows[last]);
                let last = fields.len() - 1;
                let identity = values(&mut typed(&mut fields[last])[1]);
                values(&mut identity[1])[2] = Value::Atom(target.into());
            } else {
                let body = values(&mut values(&mut values(&mut rows[last])[0])[1]);
                let identity = values(&mut values(&mut body[5])[1]);
                values(&mut identity[1])[2] = Value::Atom(target.into());
            }
        });
        assert_no_section_prefix_and_own_attributes_retained(&ds, mode);
        for id in own_ids {
            let before = baseline.iter().find(|d| &d.object_guid == id).unwrap();
            let after = ds.iter().find(|d| &d.object_guid == id).unwrap();
            assert_eq!(after.declared_types, before.declared_types, "mode {mode}");
        }
    }
}
