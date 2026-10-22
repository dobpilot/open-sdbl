use open_sdbl::metadata::{parse_config_descriptors, Value};
fn main() {
    let directory = std::env::args().nth(1).expect("нужен каталог фикстуры");
    let bytes = std::fs::read(std::path::Path::new(&directory).join("owner.deflate")).unwrap();
    let owner_id = "4132dc03-17a4-4287-b1fc-a67d57fa8ea2";
    let descriptors = parse_config_descriptors(owner_id, &bytes).unwrap();
    let descriptor = |id: &str| descriptors.iter().find(|d| d.object_guid.as_str() == id).unwrap();
    let owner = descriptor(owner_id);
    assert_eq!(owner.name, "СобытияОтправкиОтчетности");
    let expected_own = [
        ("4e0fc145-39bf-43ec-891f-1940c035a2a8", "ДатаПолучения", vec![("D", "", "")]),
        ("da01486e-1988-46e3-be8c-9ac15468f789", "Отправка", vec![("S", "", "36,1"), ("#", "cff40ee7-7d33-4ec1-9797-a9649dc2457c", "")]),
        ("fcc27e35-e9f3-401e-a34d-ad03117ad07c", "ИсходныйСтатус", vec![("#", "6139e5ae-fab3-487c-b595-d4c4eda4eaec", "")]),
        ("ad3e15c1-a137-406d-95de-b828400157ea", "Статус", vec![("#", "6139e5ae-fab3-487c-b595-d4c4eda4eaec", "")]),
        ("eb1351b0-b254-45d1-9d91-e149652c4a15", "ТребуетАудита", vec![("B", "", "")]),
        ("5c85f590-3cf2-41e7-8771-ade591340849", "АудитПроведен", vec![("B", "", "")]),
        ("3f12c2f0-c938-451f-b91a-29731174f237", "Комментарий", vec![("S", "", "")]),
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
    let section_id = "bc1b8607-1759-46c9-8ca9-d4b870ab0e96";
    let sections = owner.catalog_tabular_sections.as_ref().expect("SDK должен восстановить полный состав частей");
    assert_eq!(sections.iter().map(|g| g.as_str()).collect::<Vec<_>>(), vec![section_id]);
    let section = descriptor(section_id);
    assert_eq!(section.name, "ПричиныОтказа");
    let expected_fields = [
        ("368d07e5-e267-44fd-b9a2-d87d1073723c", "ИсходнаяПричина", "S"),
        ("3d7af490-1165-429f-8aa2-d7c0614a0170", "НеПоказывать", "B"),
        ("06aaed89-b79f-409b-921d-989a063921c4", "Причина", "S"),
        ("6d197c9c-dd3c-4802-a955-d24b83827ca2", "ПояснениеАудитора", "S"),
    ];
    assert_eq!(section.tabular_section_attributes.as_ref().unwrap().iter().map(|g|g.as_str()).collect::<Vec<_>>(), expected_fields.iter().map(|v|v.0).collect::<Vec<_>>());
    for (id, name, tag) in expected_fields { check(id, name, &[(tag, "", "")]); }
    println!("native_xml_exact=true; own=7; sections=1; fields=4; qualified_type_rows=12");
}
