mod support;
use open_sdbl::metadata::{ConfigDescriptor, Guid, Value, parse_config_descriptors};
use support::stored_deflate;
const ID: &str = "d5d8567e-ef0b-4eba-962c-1db134b6a551";
const TEXT: &str = include_str!("fixtures/characteristic-settings/attribute.config");
fn parse(text: &str) -> Vec<ConfigDescriptor> {
    parse_config_descriptors(ID, &stored_deflate(text.as_bytes())).unwrap()
}
fn owner(ds: &[ConfigDescriptor]) -> &ConfigDescriptor {
    ds.iter().find(|d| d.object_guid.as_str() == ID).unwrap()
}
#[test]
fn native_settings_and_own_attributes_match_independent_xml() {
    for (text, xml, count) in [
        (
            include_str!("fixtures/characteristic-settings/empty.config"),
            include_str!("fixtures/characteristic-settings/empty.xml"),
            0,
        ),
        (
            TEXT,
            include_str!("fixtures/characteristic-settings/attribute.xml"),
            1,
        ),
    ] {
        let ds = parse(text);
        let d = owner(&ds);
        let s = d.characteristic_settings.as_ref().unwrap();
        assert_eq!((s.code_length, s.description_length), (9, 25));
        assert!(xml.contains("<CodeLength>9</CodeLength>"));
        assert!(xml.contains("<DescriptionLength>25</DescriptionLength>"));
        assert_eq!(
            s.value_types
                .iter()
                .map(|t| t.tag.as_str())
                .collect::<Vec<_>>(),
            ["B", "S", "D", "N"]
        );
        let a = |s: &str| Value::Atom(s.into());
        assert_eq!(s.value_types[1].qualifiers, [a("100"), a("1")]);
        assert_eq!(s.value_types[2].qualifiers, []);
        assert_eq!(s.value_types[3].qualifiers, [a("15"), a("2"), a("0")]);
        for tag in ["xs:boolean", "xs:string", "xs:dateTime", "xs:decimal"] {
            assert!(xml.contains(tag));
        }
        assert_eq!(
            d.object_reference_type.as_ref().unwrap().as_str(),
            "1945502f-7d5b-4a44-b8b5-43810c645103"
        );
        assert_eq!(d.characteristic_attributes.as_ref().unwrap().len(), count);
        assert!(d.catalog_attributes.is_none());
        assert!(d.catalog_standard_attributes.is_none());
        if count == 1 {
            let id: Guid = "5babe579-8636-4733-bb83-aaf4277e198b".parse().unwrap();
            assert_eq!(d.characteristic_attributes, Some(vec![id.clone()]));
            let attr = ds.iter().find(|d| d.object_guid == id).unwrap();
            assert_eq!(attr.name, "УникальныйКодДляПоля");
            let types = attr.declared_types.as_ref().unwrap();
            assert_eq!(types.len(), 1);
            assert_eq!(types[0].tag, "S");
            assert_eq!(types[0].qualifiers, [a("12"), a("1")]);
            assert!(xml.contains(id.as_str()));
        }
    }
}
#[test]
fn unknown_types_and_incomplete_collections_do_not_publish_partial_success() {
    let bad = TEXT.replacen(
        "31182525-9346-4595-81f8-6f91a72ebe06,1",
        "31182525-9346-4595-81f8-6f91a72ebe06,2",
        1,
    );
    assert_ne!(bad, TEXT);
    let ds = parse(&bad);
    assert!(owner(&ds).characteristic_attributes.is_none());
    assert!(owner(&ds).characteristic_settings.is_some());
    let bad = TEXT.replacen("{\"S\",12,1}", "{\"?\",12,1}", 1);
    assert!(owner(&parse(&bad)).characteristic_attributes.is_none());
    let bad = TEXT.replacen("{\"B\"}", "{\"?\"}", 1);
    let ds = parse(&bad);
    assert!(owner(&ds).characteristic_settings.is_none());
    assert!(owner(&ds).characteristic_attributes.is_some());
    assert!(owner(&parse(TEXT)).characteristic_settings.is_some());
    assert!(
        parse_config_descriptors(ID, &stored_deflate(&TEXT.as_bytes()[..TEXT.len() - 1])).is_err()
    );
}
#[test]
fn malformed_native_attribute_body_and_named_attribute_cycle_are_unknown() {
    use open_sdbl::metadata::{DbNames, SchemaStorage, resolve_metadata};
    let bad = TEXT.replacen("{27,", "{28,", 1);
    assert!(owner(&parse(&bad)).characteristic_attributes.is_none());
    let mut basis = parse_config_descriptors(
        "51c403a3-e062-4f0d-b39c-11989c0ffe2b",
        include_bytes!("fixtures/runtime-metadata-096/basis.deflate"),
    )
    .unwrap()
    .into_iter()
    .find(|d| d.defined_type_id.is_some())
    .unwrap();
    basis.declared_types.as_mut().unwrap()[0].reference = basis.defined_type_id.clone();
    let text = TEXT.replacen(
        "{\"S\",12,1}",
        "{\"#\",4054e098-bf79-436d-8a4b-4e42d8f00f42}",
        1,
    );
    let mut ds = parse(&text);
    assert!(owner(&ds).characteristic_attributes.is_some());
    ds.push(basis);
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
    assert!(
        owner(snap.descriptors())
            .characteristic_attributes
            .is_none()
    );
    assert!(owner(snap.descriptors()).characteristic_settings.is_some());
}

#[test]
fn acquired_named_value_types_use_existing_expansion_and_cycle_detection() {
    use open_sdbl::metadata::{DbNames, SchemaStorage, resolve_metadata};
    let basis = parse_config_descriptors(
        "51c403a3-e062-4f0d-b39c-11989c0ffe2b",
        include_bytes!("fixtures/runtime-metadata-096/basis.deflate"),
    )
    .unwrap()
    .into_iter()
    .find(|d| d.defined_type_id.is_some())
    .unwrap();
    let text = TEXT.replacen("{\"B\"}", "{\"#\",4054e098-bf79-436d-8a4b-4e42d8f00f42}", 1);
    assert_eq!(
        owner(&parse(&text))
            .characteristic_settings
            .as_ref()
            .unwrap()
            .value_types[0]
            .reference,
        basis.defined_type_id
    );
    for cycle in [false, true] {
        let mut ds = parse(&text);
        let mut definition = basis.clone();
        if cycle {
            definition.declared_types.as_mut().unwrap()[0].reference =
                definition.defined_type_id.clone();
        }
        ds.push(definition);
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
        let d = owner(snap.descriptors());
        if cycle {
            assert!(d.characteristic_settings.is_none());
        } else {
            assert_eq!(
                d.characteristic_settings
                    .as_ref()
                    .unwrap()
                    .value_types
                    .len(),
                59
            );
        }
    }
}

#[test]
fn original_demo_uses_its_actual_attribute_type_instead_of_control_type() {
    let pack = include_bytes!("fixtures/demo/config.pack");
    let id = "1055d15b-8cb5-4ff0-a526-7fd20a08a96c";
    let mut offset = 0;
    let mut ds = None;
    while offset < pack.len() {
        let end = offset + pack[offset..].iter().position(|b| *b == b'\n').unwrap();
        let (name, n) = std::str::from_utf8(&pack[offset..end])
            .unwrap()
            .split_once('\t')
            .unwrap();
        let n: usize = n.parse().unwrap();
        offset = end + 1;
        if name == id {
            ds = Some(parse_config_descriptors(name, &pack[offset..offset + n]).unwrap());
            break;
        }
        offset += n;
    }
    let ds = ds.unwrap();
    let d = ds.iter().find(|d| d.object_guid.as_str() == id).unwrap();
    let settings = d.characteristic_settings.as_ref().unwrap();
    assert_eq!(
        (settings.code_length, settings.description_length),
        (0, 150)
    );
    let ids = d.characteristic_attributes.as_ref().unwrap();
    let a = ds
        .iter()
        .find(|a| a.name == "УникальныйКодДляПоля")
        .unwrap();
    assert!(ids.contains(&a.object_guid));
    assert_eq!(
        a.object_guid.as_str(),
        "9629483f-cd0f-4247-afdf-aa9aa1e0484e"
    );
    let types = a.declared_types.as_ref().unwrap();
    assert_eq!(types.len(), 1);
    assert_eq!(types[0].tag, "S");
    assert_eq!(
        types[0].qualifiers,
        [Value::Atom("14".into()), Value::Atom("1".into())]
    );
}
