use open_sdbl::metadata::{parse_config_descriptors, Value};
fn main() {
    let directory = std::env::args().nth(1).expect("нужен каталог фикстуры");
    let bytes = std::fs::read(std::path::Path::new(&directory).join("owner.deflate")).unwrap();
    let owner_id = "b5697377-16f3-4e8c-b65b-1742fbe55dd0";
    let descriptors = parse_config_descriptors(owner_id, &bytes).unwrap();
    let descriptor = |id: &str| descriptors.iter().find(|d| d.object_guid.as_str() == id).unwrap();
    let owner = descriptor(owner_id);
    assert_eq!(owner.name, "НаправленияСдачиОтчетности");
    let expected_own = [
        ("203addd4-6b9c-486d-a13f-cf54f3c2df06", "Организация", vec![("S", "", "12,1")]),
        ("50e82874-a444-43b9-9ba6-3fc5272bef21", "ТипНаправления", vec![("#", "8a20267d-5c6a-451a-a8a6-2774b0761f66", "")]),
        ("180cfd78-7ac3-4717-98a3-a8ad51a0d4d9", "Направление", vec![("S", "", "7,1")]),
        ("7712bed3-a048-4104-aec5-9a4702a6dd62", "ДатаАктуализации", vec![("D", "", "")]),
    ];
    let check = |id: &str, name: &str, expected: &[(&str, &str, &str)]| {
        let d = descriptor(id);
        assert_eq!(d.name, name);
        let actual = d.declared_types.as_ref().expect("нужен полный список типов").iter().map(|t| {
            let qualifiers = t.qualifiers.iter().map(|v| match v {
                Value::Atom(s) | Value::String(s) => s.as_str(),
                _ => panic!("неожиданный квалификатор"),
            }).collect::<Vec<_>>().join(",");
            (t.tag.as_str(), t.reference.as_ref().map(|g| g.as_str()).unwrap_or(""), qualifiers)
        }).collect::<Vec<_>>();
        let expected = expected.iter().map(|(t, r, q)| (*t, *r, q.to_string())).collect::<Vec<_>>();
        assert_eq!(actual, expected, "{name}");
    };
    let own = owner.catalog_attributes.as_ref().unwrap();
    assert_eq!(own.iter().map(|g| g.as_str()).collect::<Vec<_>>(), expected_own.iter().map(|v| v.0).collect::<Vec<_>>());
    for (id, name, types) in &expected_own { check(id, name, types); }
    assert_eq!(owner.catalog_tabular_sections.as_ref().unwrap().len(), 0);
    println!("native_xml_exact=true; own=4; sections=0; qualified_type_rows=4");
}
