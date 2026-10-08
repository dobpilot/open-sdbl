mod support;
use open_sdbl::metadata::{
    ConfigDescriptor, DbNames, SchemaStorage, parse_config_descriptors, resolve_metadata,
};
use support::stored_deflate;
const ID: &str = "ff92317d-71e7-4664-a13a-18bd31fd3701";
const TEXT: &str = include_str!("fixtures/data-processor-0911/Замеры.config");
fn owner<'a>(ds: &'a [ConfigDescriptor], id: &str) -> &'a ConfigDescriptor {
    ds.iter().find(|d| d.object_guid.as_str() == id).unwrap()
}
fn parse(text: &str) -> Vec<ConfigDescriptor> {
    parse_config_descriptors(ID, &stored_deflate(text.as_bytes())).unwrap()
}
#[test]
fn native_cf_members_match_independent_designer_xml_order_and_types() {
    let oracle = include_str!("fixtures/data-processor-0911/members.tsv")
        .lines()
        .map(|l| l.split('\t').collect::<Vec<_>>())
        .collect::<Vec<_>>();
    for row in oracle.iter().filter(|r| r[0] == "owner") {
        let bytes = std::fs::read(format!(
            "{}/tests/fixtures/data-processor-0911/{}.deflate",
            env!("CARGO_MANIFEST_DIR"),
            row[2]
        ))
        .unwrap();
        let ds = parse_config_descriptors(row[1], &bytes).unwrap();
        let d = owner(&ds, row[1]);
        assert_eq!(d.name, row[2]);
        assert_eq!(
            d.data_processor_object_type.as_ref().unwrap().as_str(),
            row[3]
        );
        assert_ne!(row[3], row[4]);
        assert!(d.object_reference_type.is_none());
        for (kind, actual) in [
            ("attribute", &d.data_processor_attributes),
            ("section", &d.data_processor_tabular_sections),
        ] {
            let expected = oracle
                .iter()
                .filter(|r| r[0] == kind && r[1] == row[1])
                .map(|r| r[2])
                .collect::<Vec<_>>();
            assert_eq!(
                actual
                    .as_ref()
                    .unwrap()
                    .iter()
                    .map(|g| g.as_str())
                    .collect::<Vec<_>>(),
                expected
            );
        }
        for section in d.data_processor_tabular_sections.as_ref().unwrap() {
            let s = owner(&ds, section.as_str());
            let expected = oracle
                .iter()
                .filter(|r| r[0] == "field" && r[1] == section.as_str())
                .map(|r| r[2])
                .collect::<Vec<_>>();
            assert_eq!(
                s.tabular_section_attributes
                    .as_ref()
                    .unwrap()
                    .iter()
                    .map(|g| g.as_str())
                    .collect::<Vec<_>>(),
                expected
            );
        }
        for row in oracle
            .iter()
            .filter(|r| matches!(r[0], "attribute" | "field"))
        {
            if let Some(a) = ds.iter().find(|d| d.object_guid.as_str() == row[2]) {
                assert_eq!(a.name, row[3]);
                let types = a.declared_types.as_ref().unwrap();
                assert_eq!(types.len(), 1);
                assert_eq!(
                    types[0].tag,
                    if a.name.contains("Текст") {
                        "S"
                    } else {
                        "B"
                    }
                );
                if types[0].tag == "S" {
                    assert_eq!(
                        types[0]
                            .qualifiers
                            .iter()
                            .map(|v| match v {
                                open_sdbl::metadata::Value::Atom(s) => s.as_str(),
                                _ => panic!("atom expected"),
                            })
                            .collect::<Vec<_>>(),
                        ["12", "1"]
                    );
                }
                assert!(a.fill_checking.is_some());
                assert_eq!(a.fill_from_filling_value, Some(false));
                assert!(a.filling_value.is_some());
                assert!(!a.synonyms.is_empty());
                assert!(a.data_processor_attributes.is_none());
            }
        }
        let old = d.clone();
        let snap = resolve_metadata(
            DbNames::from_entries(vec![]),
            ds,
            SchemaStorage {
                tables: vec![],
                anomalies: vec![],
            },
            vec![],
        )
        .snapshot;
        assert_eq!(&old, owner(snap.descriptors(), row[1]));
    }
}
#[test]
fn corrupted_attribute_membership_is_unknown_without_erasing_sections() {
    for (old, new) in [
        (
            "ec6bb5e5-b7a8-4d75-bec9-658107a699cf,2",
            "ec6bb5e5-b7a8-4d75-bec9-658107a699cf,3",
        ),
        (
            "4094ef40-0451-52d7-9bd7-72697aa6c353",
            "019163ac-4ca7-58a3-b494-1424ff37dcac",
        ),
    ] {
        assert!(TEXT.contains(old));
        let ds = parse(&TEXT.replacen(old, new, 1));
        let d = owner(&ds, ID);
        assert!(d.data_processor_attributes.is_none());
        assert_eq!(d.data_processor_tabular_sections.as_ref().unwrap().len(), 3);
    }
}
#[test]
fn corrupted_sections_and_unmeasured_class_never_publish_partial_membership() {
    let ds = parse(&TEXT.replacen(
        "2bcef0d1-0981-11d6-b9b8-0050bae0a95d,3",
        "2bcef0d1-0981-11d6-b9b8-0050bae0a95d,4",
        1,
    ));
    assert!(owner(&ds, ID).data_processor_tabular_sections.is_none());
    assert_eq!(
        owner(&ds, ID)
            .data_processor_attributes
            .as_ref()
            .unwrap()
            .len(),
        2
    );
    let ds = parse(&TEXT.replacen("{17,", "{999,", 1));
    assert!(owner(&ds, ID).data_processor_attributes.is_none());
    assert!(owner(&ds, ID).data_processor_tabular_sections.is_none());
    assert!(owner(&ds, ID).data_processor_object_type.is_none());
}

#[test]
fn independently_recompiled_section_fields_keep_non_alphabetic_xml_order() {
    let ds = parse_config_descriptors(
        ID,
        include_bytes!("fixtures/data-processor-0911/order.deflate"),
    )
    .unwrap();
    let oracle = include_str!("fixtures/data-processor-0911/order.tsv")
        .lines()
        .map(|l| l.split('\t').collect::<Vec<_>>())
        .collect::<Vec<_>>();
    for id in owner(&ds, ID)
        .data_processor_tabular_sections
        .as_ref()
        .unwrap()
    {
        let d = owner(&ds, id.as_str());
        let expected = oracle
            .iter()
            .filter(|r| r[0] == id.as_str())
            .map(|r| r[1])
            .collect::<Vec<_>>();
        assert_eq!(
            d.tabular_section_attributes
                .as_ref()
                .unwrap()
                .iter()
                .map(|g| g.as_str())
                .collect::<Vec<_>>(),
            expected
        );
    }
    let section = ds.iter().find(|d| d.name == "ДругаяЧасть").unwrap();
    let fields = section
        .tabular_section_attributes
        .as_ref()
        .unwrap()
        .iter()
        .map(|g| owner(&ds, g.as_str()).name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(fields, ["ДругойФлагСтроки", "ДругойТекстСтроки"]);
}
