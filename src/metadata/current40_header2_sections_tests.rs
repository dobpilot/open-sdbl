use super::*;
const ID: &str = "5090a026-1905-481c-a2ad-14615a34b4d4";
const OWN: &str = "45e46cbc-3e24-4165-8b7b-cc98a6f80211";
const PARTS: &str = "21c53e09-8950-4b5e-a6a0-1054f1bbc274";
const FIELDS: &str = "888744e1-b616-11d4-9436-004095e12fc7";
const RAW: &[u8] = include_bytes!("../../tests/fixtures/buh-equipment-installation/owner.deflate");
fn acquired() -> Vec<ConfigDescriptor> {
    super::super::parse_config_descriptors(ID, RAW).unwrap()
}
fn owner(ds: &[ConfigDescriptor]) -> &ConfigDescriptor {
    ds.iter().find(|d| d.object_guid.as_str() == ID).unwrap()
}
fn rows(s: &str) -> Vec<Vec<&str>> {
    s.lines().skip(1).map(|x| x.split('\t').collect()).collect()
}
fn values(v: &mut Value) -> &mut Vec<Value> {
    match v {
        Value::List(v) => v,
        _ => panic!("ожидается список"),
    }
}
fn collection_rows<'a>(v: &'a mut Value, id: &str) -> &'a mut Vec<Value> {
    values(v)
        .iter_mut()
        .find_map(|v| match v {
            Value::List(r) if atom(&r[0]) == Some(id) => Some(r),
            _ => None,
        })
        .unwrap()
}
fn own(v: &mut Value) -> &mut Vec<Value> {
    collection_rows(v, OWN)
}
fn parts(v: &mut Value) -> &mut Vec<Value> {
    collection_rows(v, PARTS)
}
fn fields(v: &mut Value) -> &mut Vec<Value> {
    let r = parts(v).last_mut().unwrap();
    collection_rows(r, FIELDS)
}
fn typed(v: &mut Value) -> &mut Vec<Value> {
    values(&mut values(&mut values(&mut values(v)[0])[1])[1])
}
fn identity(v: &mut Value) -> &mut Vec<Value> {
    values(&mut typed(v)[1])
}
fn part_identity(v: &mut Value) -> &mut Vec<Value> {
    let b = values(&mut values(&mut values(v)[0])[1]);
    values(&mut values(&mut b[5])[1])
}
fn owner_header(v: &mut Value) -> &mut Vec<Value> {
    values(&mut values(&mut values(v)[1])[9])
        .get_mut(1)
        .map(values)
        .unwrap()
}
fn type_rows() -> Vec<Vec<&'static str>> {
    rows(include_str!(
        "../../tests/fixtures/buh-equipment-installation/types-flat.tsv"
    ))
}
fn assert_types(ds: &[ConfigDescriptor], id: &Guid) {
    let d = ds.iter().find(|d| &d.object_guid == id).unwrap();
    let ts = d.declared_types.as_ref().unwrap();
    let all = type_rows();
    let expected = all
        .iter()
        .filter(|r| r[0] == id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ts.len(), expected.len());
    for (i, (t, r)) in ts.iter().zip(expected).enumerate() {
        assert_eq!(r[1], i.to_string());
        assert_eq!(t.tag, r[2]);
        assert_eq!(t.reference.as_ref().map(Guid::as_str).unwrap_or(""), r[3]);
        assert_eq!(
            t.qualifiers
                .iter()
                .map(|v| v.as_string().or_else(|| atom(v)).unwrap())
                .collect::<Vec<_>>()
                .join(","),
            r[4]
        );
    }
}
#[test]
fn native_equipment_matches_ten_own_two_parts_ten_fields_and_twenty_xml_types() {
    let ds = acquired();
    let own = owner(&ds);
    let ids = own.document_attributes.as_ref().unwrap();
    let expected = rows(include_str!(
        "../../tests/fixtures/buh-equipment-installation/own.tsv"
    ));
    assert_eq!(ids.len(), 10);
    for (id, r) in ids.iter().zip(expected) {
        assert_eq!(id.as_str(), r[0]);
        assert_eq!(ds.iter().find(|d| &d.object_guid == id).unwrap().name, r[1]);
        assert_types(&ds, id);
    }
    let ids = own.document_tabular_sections.as_ref().unwrap();
    let expected = rows(include_str!(
        "../../tests/fixtures/buh-equipment-installation/parts.tsv"
    ));
    assert_eq!(ids.len(), 2);
    let fs = rows(include_str!(
        "../../tests/fixtures/buh-equipment-installation/fields.tsv"
    ));
    let mut count = 0;
    for (id, r) in ids.iter().zip(expected) {
        assert_eq!(id.as_str(), r[0]);
        let p = ds.iter().find(|d| &d.object_guid == id).unwrap();
        assert_eq!(p.name, r[1]);
        let fields = p.tabular_section_attributes.as_ref().unwrap();
        let expected = fs.iter().filter(|x| x[0] == p.name).collect::<Vec<_>>();
        assert_eq!(fields.len(), r[2].parse::<usize>().unwrap());
        for (fid, fr) in fields.iter().zip(expected) {
            assert_eq!(fid.as_str(), fr[1]);
            assert_eq!(
                ds.iter().find(|d| &d.object_guid == fid).unwrap().name,
                fr[2]
            );
            assert_types(&ds, fid);
            count += 1;
        }
    }
    assert_eq!(count, 10);
    assert_eq!(type_rows().len(), 20);
}
fn mutated(
    edit: impl FnOnce(&mut Value, &mut Vec<ConfigDescriptor>),
    preserve_own: bool,
) -> Vec<ConfigDescriptor> {
    let mut ds = acquired();
    let own_ids = rows(include_str!(
        "../../tests/fixtures/buh-equipment-installation/own.tsv"
    ))
    .iter()
    .map(|r| r[0].parse::<Guid>().unwrap())
    .collect::<Vec<_>>();
    for d in &mut ds {
        d.document_tabular_sections = None;
        d.tabular_section_attributes = None;
        if !preserve_own || !own_ids.contains(&d.object_guid) {
            d.declared_types = None;
        }
        if !preserve_own {
            d.document_attributes = None;
        }
    }
    let mut v =
        super::super::parse_serialized(&super::super::inflate_raw_deflate(RAW).unwrap()).unwrap();
    edit(&mut v, &mut ds);
    project(&v, &ID.parse().unwrap(), &mut ds);
    ds
}
#[test]
fn malformed_header2_fallback_publishes_no_new_prefix_and_preserves_own_and_settings() {
    let base = acquired();
    let own_ids = owner(&base).document_attributes.clone().unwrap();
    let expected = own_ids
        .iter()
        .map(|id| {
            (
                id.clone(),
                base.iter()
                    .find(|d| &d.object_guid == id)
                    .unwrap()
                    .declared_types
                    .clone(),
            )
        })
        .collect::<Vec<_>>();
    for mode in 0..32 {
        let ds = mutated(
            |v, ds| {
                let last = fields(v).len() - 1;
                let part_id = part_identity(&mut parts(v)[3])[1].clone();
                match mode {
                    0 => parts(v)[1] = Value::Atom("3".into()),
                    1 => values(&mut parts(v)[3])[0] = Value::Null,
                    2 => part_identity(&mut parts(v)[3])[4] = Value::Atom("".into()),
                    3 => {
                        let d = ds
                            .iter()
                            .position(|d| d.name == "СведенияПрослеживаемости")
                            .unwrap();
                        ds.remove(d);
                    }
                    4 => {
                        let d = ds
                            .iter()
                            .find(|d| d.name == "СведенияПрослеживаемости")
                            .unwrap()
                            .clone();
                        ds.push(d);
                    }
                    5 => {
                        let h = part_identity(&mut parts(v)[2])[1].clone();
                        part_identity(&mut parts(v)[3])[1] = h;
                    }
                    6 => {
                        values(&mut part_identity(&mut parts(v)[3])[1])[2] = Value::Atom(ID.into())
                    }
                    7 => {
                        values(&mut part_identity(&mut parts(v)[3])[1])[2] =
                            Value::Atom(own_ids[0].to_string())
                    }
                    8 => values(&mut fields(v)[last])[0] = Value::Null,
                    9 => typed(&mut fields(v)[last])[2] = Value::Null,
                    10 => {
                        let h = identity(&mut fields(v)[last])[1].clone();
                        let id = match h {
                            Value::List(r) => guid(&r[2]).unwrap(),
                            _ => unreachable!(),
                        };
                        let i = ds.iter().position(|d| d.object_guid == id).unwrap();
                        ds.remove(i);
                    }
                    11 => {
                        let h = identity(&mut fields(v)[last])[1].clone();
                        let id = match h {
                            Value::List(r) => guid(&r[2]).unwrap(),
                            _ => unreachable!(),
                        };
                        let d = ds.iter().find(|d| d.object_guid == id).unwrap().clone();
                        ds.push(d);
                    }
                    12 => {
                        values(&mut identity(&mut fields(v)[last])[1])[2] =
                            Value::Atom(own_ids[0].to_string())
                    }
                    13 => {
                        values(&mut identity(&mut fields(v)[last])[1])[2] = Value::Atom(ID.into())
                    }
                    14 => identity(&mut fields(v)[last])[1] = part_id,
                    15 => {
                        let first = collection_rows(&mut parts(v)[2], FIELDS)[2].clone();
                        identity(&mut fields(v)[last])[1] = identity(&mut first.clone())[1].clone();
                    }
                    16 => values(&mut values(&mut fields(v)[last])[0])[2] = Value::Atom("9".into()),
                    17 => identity(&mut fields(v)[last])[4] = Value::String("comment".into()),
                    18 => fields(v)[1] = Value::Atom("5".into()),
                    19 => values(&mut parts(v)[3])[1] = Value::Atom("0".into()),
                    20 => identity(&mut fields(v)[last])[6] = Value::Atom("1".into()),
                    21 => {
                        let p = parts(v)[3].clone();
                        parts(v).push(p);
                        parts(v)[1] = Value::Atom("3".into());
                    }
                    22 => owner_header(v)[4] = Value::String("comment".into()),
                    23 => owner_header(v)[6] = Value::Atom("1".into()),
                    24 => {
                        owner_header(v)[7] =
                            Value::Atom("11111111-1111-1111-1111-111111111111".into())
                    }
                    25 => owner_header(v)[3] = Value::Null,
                    26 => {
                        identity(&mut fields(v)[last])[7] =
                            Value::Atom("11111111-1111-1111-1111-111111111111".into())
                    }
                    27 => identity(&mut fields(v)[last])[0] = Value::Atom("9".into()),
                    28 => part_identity(&mut parts(v)[3])[5] = Value::Atom("1".into()),
                    29 => part_identity(&mut parts(v)[3])[3] = Value::Null,
                    30 => identity(&mut fields(v)[last])[4] = Value::Atom("".into()),
                    31 => {
                        let h = identity(&mut fields(v)[2])[1].clone();
                        identity(&mut fields(v)[last])[1] = h;
                    }
                    _ => unreachable!(),
                }
            },
            true,
        );
        let o = owner(&ds);
        assert!(o.document_tabular_sections.is_none(), "mode {mode}");
        assert_eq!(o.document_attributes.as_ref(), Some(&own_ids));
        assert_eq!(o.document_numbering, owner(&base).document_numbering);
        assert_eq!(o.document_number, owner(&base).document_number);
        assert_eq!(o.document_registers, owner(&base).document_registers);
        for (id, t) in &expected {
            assert_eq!(
                &ds.iter()
                    .find(|d| &d.object_guid == id)
                    .unwrap()
                    .declared_types,
                t,
                "mode {mode}"
            );
        }
        for d in &ds {
            if !own_ids.contains(&d.object_guid) {
                assert!(d.declared_types.is_none(), "mode {mode}");
                assert!(d.tabular_section_attributes.is_none(), "mode {mode}");
            }
        }
    }
}
#[test]
fn the_old_some_section_route_keeps_its_original_unknown_field_metadata() {
    let raw = include_bytes!(
        "../../tests/fixtures/buh-equipment-installation/synthetic-section-tags3.deflate"
    );
    let ds = super::super::parse_config_descriptors(ID, raw).unwrap();
    let o = owner(&ds);
    assert_eq!(o.document_attributes.as_ref().unwrap().len(), 10);
    assert_eq!(o.document_tabular_sections.as_ref().unwrap().len(), 2);
    let fields = rows(include_str!(
        "../../tests/fixtures/buh-equipment-installation/fields.tsv"
    ));
    for r in fields {
        assert!(
            ds.iter()
                .find(|d| d.object_guid.as_str() == r[1])
                .unwrap()
                .declared_types
                .is_none()
        );
    }
    for id in o.document_tabular_sections.as_ref().unwrap() {
        assert!(
            ds.iter()
                .find(|d| &d.object_guid == id)
                .unwrap()
                .tabular_section_attributes
                .is_none()
        );
    }
    let base = acquired();
    assert_eq!(o.document_numbering, owner(&base).document_numbering);
    assert_eq!(o.document_number, owner(&base).document_number);
    assert_eq!(o.document_registers, owner(&base).document_registers);
}
#[test]
fn unknown_complete_own_metadata_does_not_enable_the_new_fallback() {
    let ds = mutated(|v, _| own(v)[1] = Value::Atom("11".into()), false);
    assert!(owner(&ds).document_attributes.is_none());
    assert!(owner(&ds).document_tabular_sections.is_none());
    assert!(ds.iter().all(|d| d.declared_types.is_none()));
}
