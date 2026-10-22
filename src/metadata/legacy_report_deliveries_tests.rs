use super::*;
const ID: &str = "83fa6f59-390a-4e3d-bb4a-50179b77e856";
const COLLECTION: &str = "cf4abea7-37b2-11d4-940f-008048da11f9";
const RAW: &[u8] = include_bytes!("../../tests/fixtures/buh-report-deliveries/owner.deflate");
fn acquired() -> Vec<ConfigDescriptor> {
    super::super::parse_config_descriptors(ID, RAW).unwrap()
}
fn values(v: &mut Value) -> &mut Vec<Value> {
    match v {
        Value::List(v) => v,
        _ => panic!("ожидается список"),
    }
}
fn attrs(v: &mut Value) -> &mut Vec<Value> {
    values(v)
        .iter_mut()
        .find_map(|v| match v {
            Value::List(r) if atom(&r[0]) == Some(COLLECTION) => Some(r),
            _ => None,
        })
        .unwrap()
}
fn typed(v: &mut Value) -> &mut Vec<Value> {
    values(&mut values(&mut values(&mut values(v)[0])[1])[1])
}
#[test]
fn native_report_deliveries_preserves_all_sixteen_xml_qualified_attributes() {
    let ds = acquired();
    let owner = ds.iter().find(|d| d.object_guid.as_str() == ID).unwrap();
    let ids = owner.catalog_attributes.as_ref().unwrap();
    let rows = include_str!("../../tests/fixtures/buh-report-deliveries/own-types.tsv")
        .lines()
        .skip(1)
        .map(|s| s.split('\t').collect::<Vec<_>>())
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), 16);
    assert_eq!(rows.len(), 16);
    for (id, r) in ids.iter().zip(rows) {
        assert_eq!(id.as_str(), r[0]);
        let d = ds.iter().find(|d| &d.object_guid == id).unwrap();
        assert_eq!(d.name, r[1]);
        let ts = d.declared_types.as_ref().unwrap();
        assert_eq!(ts.len(), 1);
        let t = &ts[0];
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
fn malformed_legacy_catalog_never_publishes_an_attribute_prefix() {
    for mode in 0..15 {
        let mut ds = acquired();
        for d in &mut ds {
            d.catalog_attributes = None;
            d.declared_types = None;
        }
        let mut tree =
            super::super::parse_serialized(&super::super::inflate_raw_deflate(RAW).unwrap())
                .unwrap();
        let first = typed(&mut attrs(&mut tree)[2])[1].clone();
        match mode {
            0 => attrs(&mut tree)[1] = Value::Atom("17".into()),
            1 => {
                values(&mut attrs(&mut tree)[17])[0] = Value::Null;
            }
            2 => {
                typed(&mut attrs(&mut tree)[17])[0] = Value::Atom("9".into());
            }
            3 => {
                typed(&mut attrs(&mut tree)[17])[1] = Value::Null;
            }
            4 => {
                typed(&mut attrs(&mut tree)[17])[2] = Value::Null;
            }
            5 => {
                typed(&mut attrs(&mut tree)[17])[1] = first;
            }
            6 => {
                let last = ds
                    .iter()
                    .position(|d| d.name == "УдалитьВидОтчета")
                    .unwrap();
                ds.remove(last);
            }
            7 => {
                let d = ds
                    .iter()
                    .find(|d| d.name == "УдалитьВидОтчета")
                    .unwrap()
                    .clone();
                ds.push(d);
            }
            8 => {
                let h = &mut typed(&mut attrs(&mut tree)[17])[1];
                values(&mut values(h)[1])[2] = Value::Atom(ID.into());
            }
            9 => {
                values(&mut values(&mut tree)[1])[0] = Value::Atom("54".into());
            }
            10 => {
                let class = values(&mut values(&mut tree)[1]);
                values(&mut values(&mut class[9])[1])[4] = Value::Atom("".into());
            }
            11 => {
                let class = values(&mut values(&mut tree)[1]);
                values(&mut values(&mut class[9])[1])[6] = Value::Atom("0".into());
            }
            12 => {
                let class = values(&mut values(&mut tree)[1]);
                values(&mut values(&mut class[9])[1])[3] = Value::Null;
            }
            13 => {
                let class = values(&mut values(&mut tree)[1]);
                let header = values(&mut values(&mut class[9])[1]);
                values(&mut header[3])[0] = Value::Atom("2".into());
            }
            14 => {
                let class = values(&mut values(&mut tree)[1]);
                let header = values(&mut values(&mut class[9])[1]);
                values(&mut header[3])[2] = Value::Atom("text".into());
            }
            _ => unreachable!(),
        }
        project(&tree, &ID.parse().unwrap(), &mut ds);
        assert!(
            ds.iter()
                .find(|d| d.object_guid.as_str() == ID)
                .unwrap()
                .catalog_attributes
                .is_none(),
            "mode {mode}"
        );
        assert!(ds.iter().all(|d| d.declared_types.is_none()), "mode {mode}");
    }
}
