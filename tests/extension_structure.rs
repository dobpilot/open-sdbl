//! The structure a configuration extension declares: the catalog of its
//! configuration and the headers of the objects it describes. The fixtures
//! are the `ConfigCAS` rows of `Расширение1` and `Расширение2` of the demo
//! base (`tests/fixtures/extension/demo_store`).

use std::path::PathBuf;
use std::str::FromStr;

use open_sdbl::metadata::{
    ExtensionObject, Guid, ObjectBelonging, extension_configuration_root, inflate_raw_deflate,
    parse_extension_catalog, parse_extension_info, parse_extension_objects,
};

/// The root index of `Расширение1`.
const ROOT: &str = "89c0c813d3b9d05652ec8cef91e2ff6f1313f3e0";
/// Its configuration root.
const CONFIGURATION_ROOT: &str = "8f815a90f5addb4fb9756263be3af6f8682e5b8a";
/// `СтавкиНДС`, adopted from the main configuration.
const ADOPTED_CATALOG: &str = "33b58e119708b04f9eaa8a97133c443cefa7a3d3";
/// `Расш1_Справочник1`, the extension's own catalog.
const OWN_CATALOG: &str = "3a5bbff87a72dcc28135e9302e24c0006d23389f";

const CATALOGS: &str = "cf4abea6-37b2-11d4-940f-008048da11f9";
const ROLES: &str = "09736b02-9cac-4e3f-b4f7-d3e9576ab948";
const LANGUAGES: &str = "9cd510ce-abfc-11d4-9434-004095e12fc7";

fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/extension")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn stored(key: &str) -> Vec<u8> {
    inflate_raw_deflate(&fixture(&format!("demo_store/{key}.deflate"))).unwrap()
}

fn guid(text: &str) -> Guid {
    Guid::from_str(text).unwrap()
}

fn object<'objects>(objects: &'objects [ExtensionObject], name: &str) -> &'objects ExtensionObject {
    objects
        .iter()
        .find(|object| object.name == name)
        .unwrap_or_else(|| panic!("no header names {name}"))
}

#[test]
fn the_root_index_names_the_configuration_root() {
    assert_eq!(
        extension_configuration_root(&stored(ROOT)).unwrap(),
        Some(guid("182ffc1d-7701-4067-a547-c7b78532e40d"))
    );
    // The older fixture of `_ДемоРасширение` names its root the same way,
    // with an empty third item.
    let older = inflate_raw_deflate(&fixture(
        "root_344e701a5292613d188f54a0461ba28cdc4e64a0.deflate",
    ))
    .unwrap();
    assert_eq!(
        extension_configuration_root(&older).unwrap(),
        Some(guid("2fd73213-5b64-4002-b842-6e6dbd6ab9fa"))
    );
}

#[test]
fn a_root_index_without_the_record_names_no_configuration_root() {
    let index =
        br#"{2,"shared.0",AAAAAAAAAAAAAAAAAAAAAAAAAAA=,"own.0",AAAAAAAAAAAAAAAAAAAAAAAAAAA=}"#;
    assert_eq!(extension_configuration_root(index).unwrap(), None);
}

#[test]
fn decodes_the_catalog_of_the_configuration() {
    let catalog = parse_extension_catalog(&stored(CONFIGURATION_ROOT)).unwrap();
    assert_eq!(
        catalog.configuration,
        guid("182ffc1d-7701-4067-a547-c7b78532e40d")
    );
    let objects = |class: &str| {
        catalog
            .classes
            .iter()
            .find(|entry| entry.class == guid(class))
            .unwrap_or_else(|| panic!("no class {class}"))
            .objects
            .clone()
    };
    assert_eq!(
        objects(CATALOGS),
        [
            guid("e1945025-1979-4ad7-8f06-3cf53c99530a"),
            guid("b7608864-b9ec-42d9-abb2-e7d7b0bdd348"),
        ]
    );
    assert_eq!(
        objects(ROLES),
        [guid("c4478a85-7b98-42ce-a25f-1abff2d0a125")]
    );
    assert_eq!(
        objects(LANGUAGES),
        [guid("88d12ead-1cc1-4d46-8657-19c1a2f859fc")]
    );
    // Every class is kept, the ones listing nothing included.
    assert!(catalog.classes.len() > 40, "{}", catalog.classes.len());
    assert!(catalog.classes.iter().any(|entry| entry.objects.is_empty()));
}

#[test]
fn the_catalog_lists_every_described_object() {
    // Each object the catalog lists is a resource the root index names.
    let catalog = parse_extension_catalog(&stored(CONFIGURATION_ROOT)).unwrap();
    let index = String::from_utf8(stored(ROOT)).unwrap();
    for object in catalog.classes.iter().flat_map(|entry| &entry.objects) {
        assert!(
            index.contains(&format!("\"{object}\"")),
            "{object} is not in the root index"
        );
    }
}

#[test]
fn refuses_an_entry_counting_what_it_does_not_hold() {
    let malformed = b"{2,{182ffc1d-7701-4067-a547-c7b78532e40d},1,\
        {cf4abea6-37b2-11d4-940f-008048da11f9,{1,{0},1,\
        {cf4abea6-37b2-11d4-940f-008048da11f9,2,e1945025-1979-4ad7-8f06-3cf53c99530a}}}}";
    let error = parse_extension_catalog(malformed).unwrap_err().to_string();
    assert!(error.contains("counts 2 items it does not hold"), "{error}");

    let overfull = b"{2,{182ffc1d-7701-4067-a547-c7b78532e40d},1,\
        {cf4abea6-37b2-11d4-940f-008048da11f9,{1,{0},1,\
        {cf4abea6-37b2-11d4-940f-008048da11f9,0,e1945025-1979-4ad7-8f06-3cf53c99530a}}}}";
    let error = parse_extension_catalog(overfull).unwrap_err().to_string();
    assert!(error.contains("holds more than it counts"), "{error}");
}

#[test]
fn an_adopted_catalog_carries_its_link_to_the_base() {
    let objects = parse_extension_objects(&stored(ADOPTED_CATALOG)).unwrap();

    let catalog = object(&objects, "СтавкиНДС");
    assert_eq!(catalog.id, guid("e1945025-1979-4ad7-8f06-3cf53c99530a"));
    assert_eq!(catalog.belonging, Some(ObjectBelonging::Adopted));
    // `Reference14574` of the base's DBNames.
    assert_eq!(
        catalog.extends,
        Some(guid("6a386de9-b78e-47cb-a7e8-864b52022b24"))
    );

    let attribute = object(&objects, "Ставка");
    assert_eq!(attribute.belonging, Some(ObjectBelonging::Adopted));
    // `Fld14578` of the base's DBNames.
    assert_eq!(
        attribute.extends,
        Some(guid("9a871782-6f75-4078-a8f2-a33d9dcb8c7f"))
    );

    for own in ["Расш1_Реквизит1", "Расш1_Реквизит2", "Расш1_Реквизит3"]
    {
        let own = object(&objects, own);
        assert_eq!(own.belonging, Some(ObjectBelonging::Own), "{}", own.name);
        assert_eq!(own.extends, None, "{}", own.name);
    }
}

#[test]
fn an_object_of_the_extension_s_own_has_no_link() {
    let objects = parse_extension_objects(&stored(OWN_CATALOG)).unwrap();
    let catalog = object(&objects, "Расш1_Справочник1");
    assert_eq!(catalog.id, guid("b7608864-b9ec-42d9-abb2-e7d7b0bdd348"));
    assert_eq!(catalog.belonging, Some(ObjectBelonging::Own));
    assert_eq!(catalog.extends, None);
}

#[test]
fn the_configuration_itself_is_adopted_without_a_link() {
    let objects = parse_extension_objects(&stored(CONFIGURATION_ROOT)).unwrap();
    let configuration = object(&objects, "Расширение1");
    assert_eq!(configuration.belonging, Some(ObjectBelonging::Adopted));
    assert_eq!(configuration.extends, None);
}

#[test]
fn an_object_adopted_by_name_has_no_link() {
    // The header of an adopted attribute of the `buh` add-on, which maps
    // by name: the extended identifier is nil.
    let header = r#"{3,{1,0,d1ab7671-2a58-4a4a-9d9c-6a1c0a9fbcf1},"Банк",{0},"",1,1,b1053250-abe6-11d4-9434-004095e12fc7,2,00000000-0000-0000-0000-000000000000,0}"#;
    let objects = parse_extension_objects(header.as_bytes()).unwrap();
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].belonging, Some(ObjectBelonging::Adopted));
    assert_eq!(objects[0].extends, None);
}

#[test]
fn an_unfamiliar_tail_answers_unknown_rather_than_a_guess() {
    for header in [
        // Two pairs counted, one present.
        r#"{3,{1,0,e1945025-1979-4ad7-8f06-3cf53c99530a},"СтавкиНДС",{0},"",1,2,9595ddd6-e72c-47ad-a156-672db811628c,2,6a386de9-b78e-47cb-a7e8-864b52022b24,0}"#,
        // A belonging value never measured.
        r#"{3,{1,0,e1945025-1979-4ad7-8f06-3cf53c99530a},"СтавкиНДС",{0},"",2,1,9595ddd6-e72c-47ad-a156-672db811628c,2,6a386de9-b78e-47cb-a7e8-864b52022b24,0}"#,
        // A tail that stops after the belonging.
        r#"{3,{1,0,e1945025-1979-4ad7-8f06-3cf53c99530a},"СтавкиНДС",{0},"",1}"#,
    ] {
        let objects = parse_extension_objects(header.as_bytes()).unwrap();
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].name, "СтавкиНДС");
        assert_eq!(objects[0].id, guid("e1945025-1979-4ad7-8f06-3cf53c99530a"));
        assert_eq!(objects[0].belonging, None);
        assert_eq!(objects[0].extends, None);
    }
}

#[test]
fn every_description_of_both_extensions_decodes_in_the_measured_shape() {
    let directory =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/extension/demo_store");
    let mut headers = 0;
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path
            .extension()
            .is_none_or(|extension| extension != "deflate")
        {
            continue;
        }
        let text = inflate_raw_deflate(&std::fs::read(&path).unwrap()).unwrap();
        for object in parse_extension_objects(&text).unwrap() {
            headers += 1;
            assert!(
                object.belonging.is_some(),
                "{}: {} is not in the measured shape",
                path.display(),
                object.name
            );
        }
    }
    assert_eq!(headers, 12);
}

#[test]
fn the_version_an_extension_record_carries() {
    let versioned = parse_extension_info(&fixture("extension_info_versioned.bin")).unwrap();
    assert_eq!(versioned.version.as_deref(), Some("2.4"));
    assert_eq!(versioned.active, Some(true));

    // The demo extensions carry no version: it is absent, not empty.
    let unversioned = parse_extension_info(&fixture("extension_info_applied.bin")).unwrap();
    assert_eq!(unversioned.version, None);
}
