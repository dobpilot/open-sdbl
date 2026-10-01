mod support;

use open_sdbl::metadata::{
    AttributeIndexing, CatalogCodeType, ConfigFillingValue, Guid, inflate_raw_deflate,
    parse_config_descriptors,
};
use std::collections::HashMap;
use support::stored_deflate;

const EVENT: &str = "571e1bd9-0f64-4a05-a276-dba4fe242ad0";
const BASIS: &str = "51c403a3-e062-4f0d-b39c-11989c0ffe2b";
const EVENT_BYTES: &[u8] = include_bytes!("fixtures/runtime-metadata-096/event.deflate");

#[test]
fn settings_and_members_match_every_uuid_in_the_native_xml_corpus() {
    let expected = include_str!("fixtures/runtime-metadata-096/unf-native.tsv")
        .lines()
        .map(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            (fields[0], fields)
        })
        .collect::<HashMap<_, _>>();
    let pack = include_bytes!("fixtures/unf/config.pack");
    let mut offset = 0;
    let (mut catalogs, mut docs, mut attrs) = (0, 0, 0);
    while offset < pack.len() {
        let end = offset + pack[offset..].iter().position(|b| *b == b'\n').unwrap();
        let (id, n) = std::str::from_utf8(&pack[offset..end])
            .unwrap()
            .split_once('\t')
            .unwrap();
        let n: usize = n.parse().unwrap();
        offset = end + 1;
        if let Some(fields) = expected.get(id) {
            let descriptors = parse_config_descriptors(id, &pack[offset..offset + n]).unwrap();
            let owner = descriptors
                .iter()
                .find(|d| d.object_guid.as_str() == id)
                .unwrap();
            let ids = |s: &str| {
                s.split(',')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.parse::<Guid>().unwrap())
                    .collect::<Vec<_>>()
            };
            assert_eq!(owner.name, fields[2]);
            if fields[1] == "Catalogs" {
                assert_eq!(owner.catalog_owners, Some(ids(fields[3])), "{}", owner.name);
                catalogs += 1;
            } else {
                let settings = fields[3].split(',').collect::<Vec<_>>();
                let number = owner.document_number.unwrap();
                assert_eq!(number.number_type, CatalogCodeType::String);
                assert_eq!(number.length.to_string(), settings[1]);
                assert_eq!(number.variable_length, settings[2] == "Variable");
                assert_eq!(
                    owner.document_attributes,
                    Some(ids(fields[4])),
                    "{}",
                    owner.name
                );
                assert_eq!(
                    owner.document_tabular_sections,
                    Some(ids(fields[5])),
                    "{}",
                    owner.name
                );
                docs += 1;
            }
            for id in ids(fields[4]) {
                let descriptor = descriptors.iter().find(|d| d.object_guid == id).unwrap();
                let at = &expected[id.as_str()];
                assert_eq!(
                    descriptor.attribute_indexing,
                    Some(match at[3] {
                        "DontIndex" => AttributeIndexing::Disabled,
                        "Index" => AttributeIndexing::Enabled,
                        "IndexWithAdditionalOrder" => AttributeIndexing::WithAdditionalOrder,
                        _ => panic!(),
                    })
                );
                assert_eq!(descriptor.fill_from_filling_value, Some(at[4] == "1"));
                assert!(descriptor.declared_types.is_some(), "{}", descriptor.name);
                attrs += 1;
            }
        }
        offset += n;
    }
    assert_eq!((catalogs, docs, attrs), (296, 125, 7541));
}

#[test]
fn event_retains_typed_filling_values_and_named_reference_identity() {
    let descriptors = parse_config_descriptors(EVENT, EVENT_BYTES).unwrap();
    let owner = descriptors
        .iter()
        .find(|d| d.object_guid.as_str() == EVENT)
        .unwrap();
    let ids = owner.document_attributes.as_ref().unwrap();
    let field = |name: &str| {
        descriptors
            .iter()
            .find(|d| d.name == name && ids.contains(&d.object_guid))
            .unwrap()
    };
    assert_eq!(
        field("Автор").filling_value,
        Some(ConfigFillingValue::Undefined)
    );
    assert_eq!(
        field("КонтактыПеренесены").filling_value,
        Some(ConfigFillingValue::Boolean(false))
    );
    assert_eq!(
        field("Важность").filling_value,
        Some(ConfigFillingValue::Reference {
            type_id: "981c7e3c-694b-4328-8937-217bb303823c".parse().unwrap(),
            value_id: "e2942d8d-433f-497b-8b7e-c999fbc88b44".parse().unwrap()
        })
    );
    assert_eq!(
        field("ДокументОснование").declared_types.as_ref().unwrap()[0].reference,
        Some("4054e098-bf79-436d-8a4b-4e42d8f00f42".parse().unwrap())
    );
    assert_eq!(
        field("КартинкиHTML").declared_types.as_ref().unwrap()[0].reference,
        Some("e199ca70-93cf-46ce-a54b-6edc88c3a296".parse().unwrap())
    );
    let basis = parse_config_descriptors(
        BASIS,
        include_bytes!("fixtures/runtime-metadata-096/basis.deflate"),
    )
    .unwrap();
    let basis = basis
        .iter()
        .find(|d| d.object_guid.as_str() == BASIS)
        .unwrap();
    assert_eq!(
        basis.defined_type_id,
        Some("4054e098-bf79-436d-8a4b-4e42d8f00f42".parse().unwrap())
    );
    assert_eq!(basis.declared_types.as_ref().unwrap().len(), 56);
    assert!(
        basis
            .declared_types
            .as_ref()
            .unwrap()
            .iter()
            .all(|t| t.reference.is_some())
    );
}

#[test]
fn malformed_collection_and_unknown_fill_value_are_not_guessed() {
    let decoded = inflate_raw_deflate(EVENT_BYTES).unwrap();
    let text = std::str::from_utf8(&decoded).unwrap();
    let invalid = text.replacen(
        "45e46cbc-3e24-4165-8b7b-cc98a6f80211,26",
        "45e46cbc-3e24-4165-8b7b-cc98a6f80211,27",
        1,
    );
    let ds = parse_config_descriptors(EVENT, &stored_deflate(invalid.as_bytes())).unwrap();
    let owner = ds.iter().find(|d| d.object_guid.as_str() == EVENT).unwrap();
    assert!(owner.document_attributes.is_none());
    let invalid = text.replace(
        "5c14e26f-099b-4d37-84a6-b433d87400da",
        "5c14e26f-099b-4d37-84a6-b433d87400db",
    );
    let ds = parse_config_descriptors(EVENT, &stored_deflate(invalid.as_bytes())).unwrap();
    assert!(
        ds.iter()
            .find(|d| d.name == "Важность")
            .unwrap()
            .filling_value
            .is_none()
    );
}

#[test]
fn snapshot_expands_all_named_members_and_rejects_cycles_or_duplicates() {
    use open_sdbl::metadata::{DbNames, SchemaStorage, resolve_metadata};
    let mut descriptors = parse_config_descriptors(EVENT, EVENT_BYTES).unwrap();
    let basis = parse_config_descriptors(
        BASIS,
        include_bytes!("fixtures/runtime-metadata-096/basis.deflate"),
    )
    .unwrap()
    .into_iter()
    .find(|d| d.object_guid.as_str() == BASIS)
    .unwrap();
    descriptors.push(basis.clone());
    let snapshot = resolve_metadata(
        DbNames::from_entries(Vec::new()),
        descriptors,
        SchemaStorage {
            tables: Vec::new(),
            anomalies: Vec::new(),
        },
        Vec::new(),
    )
    .snapshot;
    let field = snapshot
        .descriptors()
        .iter()
        .find(|d| d.object_guid.as_str() == "e350fdab-d2d1-45e8-bd75-a2ee410245f1")
        .unwrap();
    assert_eq!(field.declared_types.as_ref().unwrap().len(), 56);
    assert_eq!(field.reference_types.len(), 56);
    for duplicate in [false, true] {
        let mut descriptors = parse_config_descriptors(EVENT, EVENT_BYTES).unwrap();
        let mut invalid = basis.clone();
        if duplicate {
            descriptors.push(invalid.clone());
        } else {
            invalid.declared_types.as_mut().unwrap()[0].reference = invalid.defined_type_id.clone();
        }
        descriptors.push(invalid);
        let snapshot = resolve_metadata(
            DbNames::from_entries(Vec::new()),
            descriptors,
            SchemaStorage {
                tables: Vec::new(),
                anomalies: Vec::new(),
            },
            Vec::new(),
        )
        .snapshot;
        let field = snapshot
            .descriptors()
            .iter()
            .find(|d| d.object_guid.as_str() == "e350fdab-d2d1-45e8-bd75-a2ee410245f1")
            .unwrap();
        assert!(field.declared_types.is_none());
        assert!(field.reference_types.is_empty());
    }
}

#[test]
fn native_number_variants_and_known_empty_document_collections_are_preserved() {
    for (id, bytes, ty, length, variable) in [
        (
            "4dbedad1-342b-54c1-9bcf-1d79091376ef",
            include_bytes!("fixtures/runtime-metadata-096/ProbeString.deflate").as_slice(),
            CatalogCodeType::String,
            9,
            true,
        ),
        (
            "c96786e5-f0b2-51f7-85a5-5d3bcbff2410",
            include_bytes!("fixtures/runtime-metadata-096/ProbeNumeric.deflate").as_slice(),
            CatalogCodeType::Number,
            9,
            false,
        ),
        (
            "6080fb5a-cf3e-599a-a005-37de07820830",
            include_bytes!("fixtures/runtime-metadata-096/ProbeDisabled.deflate").as_slice(),
            CatalogCodeType::String,
            0,
            true,
        ),
    ] {
        let ds = parse_config_descriptors(id, bytes).unwrap();
        let owner = ds.iter().find(|d| d.object_guid.as_str() == id).unwrap();
        let number = owner.document_number.unwrap();
        assert_eq!(
            (number.number_type, number.length, number.variable_length),
            (ty, length, variable)
        );
        assert_eq!(owner.document_attributes, Some(Vec::new()));
        assert_eq!(owner.document_tabular_sections, Some(Vec::new()));
    }
}
