use super::*;
const ID: &str = "4132dc03-17a4-4287-b1fc-a67d57fa8ea2";
const PART_ID: &str = "bc1b8607-1759-46c9-8ca9-d4b870ab0e96";
const PARTS: &str = "932159f9-95b2-4e76-a8dd-8849fe5c5ded";
const FIELDS: &str = "888744e1-b616-11d4-9436-004095e12fc7";
const RAW: &[u8] = include_bytes!("../../tests/fixtures/buh-report-event-sections/owner.deflate");
fn acquired() -> Vec<ConfigDescriptor> {
    super::super::parse_config_descriptors(ID, RAW).unwrap()
}
fn owner(ds: &[ConfigDescriptor]) -> &ConfigDescriptor {
    ds.iter().find(|d| d.object_guid.as_str() == ID).unwrap()
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
fn parts(v: &mut Value) -> &mut Vec<Value> {
    collection_rows(v, PARTS)
}
fn fields(v: &mut Value) -> &mut Vec<Value> {
    collection_rows(&mut parts(v)[2], FIELDS)
}
fn typed(v: &mut Value) -> &mut Vec<Value> {
    values(&mut values(&mut values(&mut values(v)[0])[1])[1])
}
fn identity(v: &mut Value) -> &mut Vec<Value> {
    values(&mut typed(v)[1])
}
#[test]
fn native_report_event_section_preserves_four_ordered_xml_fields_and_own_attributes() {
    let ds = acquired();
    let own = owner(&ds);
    assert_eq!(own.catalog_attributes.as_ref().unwrap().len(), 7);
    let ids = own.catalog_tabular_sections.as_ref().unwrap();
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0].as_str(), PART_ID);
    let section = ds.iter().find(|d| d.object_guid == ids[0]).unwrap();
    assert_eq!(section.name, "ПричиныОтказа");
    let fields = section.tabular_section_attributes.as_ref().unwrap();
    let rows =
        include_str!("../../tests/fixtures/buh-report-event-sections/section-types-flat.tsv")
            .lines()
            .skip(1)
            .map(|s| s.split('\t').collect::<Vec<_>>())
            .collect::<Vec<_>>();
    assert_eq!(fields.len(), 4);
    assert_eq!(rows.len(), 4);
    for (id, row) in fields.iter().zip(rows) {
        assert_eq!(id.as_str(), row[0]);
        let d = ds.iter().find(|d| &d.object_guid == id).unwrap();
        assert_eq!(d.name, row[1]);
        let ts = d.declared_types.as_ref().unwrap();
        assert_eq!(ts.len(), 1);
        let t = &ts[0];
        assert_eq!(t.tag, row[2]);
        assert_eq!(t.reference.as_ref().map(Guid::as_str).unwrap_or(""), row[3]);
        assert_eq!(
            t.qualifiers
                .iter()
                .map(|v| v.as_string().or_else(|| atom(v)).unwrap())
                .collect::<Vec<_>>()
                .join(","),
            row[4]
        );
    }
    for id in own.catalog_attributes.as_ref().unwrap() {
        assert!(
            ds.iter()
                .find(|d| &d.object_guid == id)
                .unwrap()
                .declared_types
                .is_some()
        );
    }
}
#[test]
fn invalid_late_catalog_sections_never_publish_prefixes_or_overwrite_own_attributes() {
    for mode in 0..21 {
        let mut ds = acquired();
        let own_ids = owner(&ds).catalog_attributes.clone().unwrap();
        let expected = own_ids
            .iter()
            .map(|id| {
                (
                    id.clone(),
                    ds.iter()
                        .find(|d| &d.object_guid == id)
                        .unwrap()
                        .declared_types
                        .clone(),
                )
            })
            .collect::<Vec<_>>();
        for d in &mut ds {
            d.catalog_tabular_sections = None;
            d.tabular_section_attributes = None;
            if !own_ids.contains(&d.object_guid) {
                d.declared_types = None;
            }
        }
        let mut tree =
            super::super::parse_serialized(&super::super::inflate_raw_deflate(RAW).unwrap())
                .unwrap();
        let first = identity(&mut fields(&mut tree)[2]).clone();
        match mode {
            0 => parts(&mut tree)[1] = Value::Atom("2".into()),
            1 => collection_rows(&mut parts(&mut tree)[2], FIELDS)[1] = Value::Atom("5".into()),
            2 => values(&mut fields(&mut tree)[5])[0] = Value::Null,
            3 => typed(&mut fields(&mut tree)[5])[0] = Value::Atom("3".into()),
            4 => typed(&mut fields(&mut tree)[5])[2] = Value::Null,
            5 => typed(&mut fields(&mut tree)[5])[1] = Value::List(first),
            6 => {
                let i = ds
                    .iter()
                    .position(|d| d.name == "ПояснениеАудитора")
                    .unwrap();
                ds.remove(i);
            }
            7 => {
                let d = ds
                    .iter()
                    .find(|d| d.name == "ПояснениеАудитора")
                    .unwrap()
                    .clone();
                ds.push(d);
            }
            8 => values(&mut identity(&mut fields(&mut tree)[5])[1])[2] = Value::Atom(ID.into()),
            9 => {
                values(&mut identity(&mut fields(&mut tree)[5])[1])[2] =
                    Value::Atom(own_ids[0].to_string())
            }
            10 => {
                let row = values(&mut parts(&mut tree)[2]);
                let body = values(&mut values(&mut row[0])[1]);
                let h = values(&mut values(&mut body[5])[1]);
                values(&mut h[1])[2] = Value::Atom(ID.into());
            }
            11 => {
                let row = values(&mut parts(&mut tree)[2]);
                let body = values(&mut values(&mut row[0])[1]);
                let h = values(&mut values(&mut body[5])[1]);
                values(&mut h[1])[2] = Value::Atom(own_ids[0].to_string());
            }
            12 => {
                let mut late = parts(&mut tree)[2].clone();
                values(&mut late)[0] = Value::Null;
                parts(&mut tree).push(late);
                parts(&mut tree)[1] = Value::Atom("2".into());
            }
            13 => {
                let late = parts(&mut tree)[2].clone();
                parts(&mut tree).push(late);
                parts(&mut tree)[1] = Value::Atom("2".into());
            }
            14 => {
                let i = ds
                    .iter()
                    .position(|d| d.object_guid.as_str() == PART_ID)
                    .unwrap();
                ds.remove(i);
            }
            15 => values(&mut values(&mut fields(&mut tree)[5])[0])[2] = Value::Atom("9".into()),
            16 => values(&mut values(&mut parts(&mut tree)[2])[0])[2] = Value::Atom("9".into()),
            17 => identity(&mut fields(&mut tree)[5])[4] = Value::Atom("".into()),
            18 => {
                typed(&mut fields(&mut tree)[5])[2] = Value::List(vec![
                    Value::String("Pattern".into()),
                    Value::List(vec![Value::String("UNKNOWN".into())]),
                ])
            }
            19 => identity(&mut fields(&mut tree)[5])[6] = Value::Atom("1".into()),
            20 => {
                values(&mut identity(&mut fields(&mut tree)[5])[1])[2] = Value::Atom(PART_ID.into())
            }
            _ => unreachable!(),
        }
        project(&tree, &ID.parse().unwrap(), &mut ds);
        assert!(owner(&ds).catalog_tabular_sections.is_none(), "mode {mode}");
        assert_eq!(
            owner(&ds).catalog_attributes.as_ref(),
            Some(&own_ids),
            "mode {mode}"
        );
        for (id, types) in &expected {
            assert_eq!(
                &ds.iter()
                    .find(|d| &d.object_guid == id)
                    .unwrap()
                    .declared_types,
                types,
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
fn earlier_native_report_deliveries_section_uses_the_same_catalog_profile() {
    let raw = include_bytes!("../../tests/fixtures/buh-report-deliveries/owner.deflate");
    let ds = super::super::parse_config_descriptors("83fa6f59-390a-4e3d-bb4a-50179b77e856", raw)
        .unwrap();
    let own = ds
        .iter()
        .find(|d| d.object_guid.as_str() == "83fa6f59-390a-4e3d-bb4a-50179b77e856")
        .unwrap();
    assert_eq!(own.catalog_attributes.as_ref().unwrap().len(), 16);
    let ids = own.catalog_tabular_sections.as_ref().unwrap();
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0].as_str(), "12044d17-64b7-4a35-b7bc-0e80b58c289a");
    let part = ds.iter().find(|d| d.object_guid == ids[0]).unwrap();
    let fields = part.tabular_section_attributes.as_ref().unwrap();
    assert_eq!(fields.len(), 2);
    for (id, (expected_id, name)) in fields.iter().zip([
        ("601f2c3d-5c8c-413d-927f-bd1ce37ed485", "Причина"),
        ("0ad4e958-8449-4a16-873d-4424d0e2219d", "Пояснение"),
    ]) {
        assert_eq!(id.as_str(), expected_id);
        let d = ds.iter().find(|d| &d.object_guid == id).unwrap();
        assert_eq!(d.name, name);
        let types = d.declared_types.as_ref().unwrap();
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].tag, "S");
        assert!(types[0].reference.is_none());
        assert!(types[0].qualifiers.is_empty());
    }
}
