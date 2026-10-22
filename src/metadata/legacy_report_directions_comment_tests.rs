use super::*;
const ID: &str = "b5697377-16f3-4e8c-b65b-1742fbe55dd0";
const RAW: &[u8] =
    include_bytes!("../../tests/fixtures/buh-report-directions-comment/owner.deflate");
fn acquired() -> Vec<ConfigDescriptor> {
    super::super::parse_config_descriptors(ID, RAW).unwrap()
}
fn values(v: &mut Value) -> &mut Vec<Value> {
    match v {
        Value::List(v) => v,
        _ => panic!("ожидается список"),
    }
}
fn header(v: &mut Value) -> &mut Vec<Value> {
    values(&mut values(&mut values(v)[1])[9])
        .get_mut(1)
        .map(values)
        .unwrap()
}
fn projected(edit: impl FnOnce(&mut Value)) -> Vec<ConfigDescriptor> {
    let mut tree =
        super::super::parse_serialized(&super::super::inflate_raw_deflate(RAW).unwrap()).unwrap();
    let mut ds = acquired();
    for d in &mut ds {
        d.catalog_attributes = None;
        d.catalog_tabular_sections = None;
        d.declared_types = None;
    }
    edit(&mut tree);
    project(&tree, &ID.parse().unwrap(), &mut ds);
    ds
}
fn assert_xml(ds: &[ConfigDescriptor]) {
    let owner = ds.iter().find(|d| d.object_guid.as_str() == ID).unwrap();
    let ids = owner.catalog_attributes.as_ref().unwrap();
    assert_eq!(ids.len(), 4);
    assert_eq!(owner.catalog_tabular_sections.as_ref().unwrap().len(), 0);
    let rows =
        include_str!("../../tests/fixtures/buh-report-directions-comment/own-types-flat.tsv")
            .lines()
            .skip(1)
            .map(|s| s.split('\t').collect::<Vec<_>>())
            .collect::<Vec<_>>();
    assert_eq!(rows.len(), 4);
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
fn native_reporting_directions_textual_owner_comment_preserves_exact_xml_metadata() {
    let text = super::super::inflate_raw_deflate(RAW).unwrap();
    let mut tree = super::super::parse_serialized(&text).unwrap();
    assert_eq!(
        header(&mut tree)[4].as_string(),
        Some("Сведения о доступных направлениях сдачи отчетности")
    );
    assert_xml(&acquired());
}
#[test]
fn previously_supported_empty_string_comment_retains_the_same_four_declarations() {
    let ds = projected(|v| header(v)[4] = Value::String(String::new()));
    assert_xml(&ds);
}
#[test]
fn nonstring_owner_comments_and_unrelated_invalid_guards_remain_unrecognized() {
    for mode in 0..7 {
        let ds = projected(|v| match mode {
            0 => header(v)[4] = Value::Atom(String::new()),
            1 => header(v)[4] = Value::Atom("comment".into()),
            2 => header(v)[4] = Value::Null,
            3 => header(v)[4] = Value::List(vec![]),
            4 => header(v)[4] = Value::List(vec![Value::String("comment".into())]),
            5 => header(v)[6] = Value::Atom("0".into()),
            6 => {
                let root = values(v);
                let rows = values(&mut root[6]);
                let row = values(&mut rows[5]);
                let wrapper = values(&mut row[0]);
                let body = values(&mut wrapper[1]);
                let typed = values(&mut body[1]);
                values(&mut typed[1])[4] = Value::String("unmeasured child comment".into());
            }
            _ => unreachable!(),
        });
        let owner = ds.iter().find(|d| d.object_guid.as_str() == ID).unwrap();
        assert!(owner.catalog_attributes.is_none(), "mode {mode}");
        assert!(owner.catalog_tabular_sections.is_none(), "mode {mode}");
        assert!(ds.iter().all(|d| d.declared_types.is_none()), "mode {mode}");
    }
}
