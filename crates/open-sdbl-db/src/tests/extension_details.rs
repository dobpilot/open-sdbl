//! Tests of what a whole-configuration read answers about each extension
//! beyond its resources, over the `ConfigCAS` and `_ExtensionsInfo` rows of
//! the demo base (`tests/fixtures/extension/demo_store`): `Расширение1`,
//! which the base applies, and `Расширение2`, which it does not.

use std::path::PathBuf;
use std::str::FromStr;

use open_sdbl::metadata::{Guid, ObjectBelonging};

use super::configuration_tests::FakeSource;
use super::{AcquiredExtension, ExtensionCatalogRow, acquire_configuration};
use crate::hex_test_support::hex;
use crate::progress::NoProgress;

const CATALOGS: &str = "cf4abea6-37b2-11d4-940f-008048da11f9";
/// The key of the configuration root of `Расширение1`.
const CONFIGURATION_ROOT: &str = "8f815a90f5addb4fb9756263be3af6f8682e5b8a";

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/extension")
}

/// The rows of `_ExtensionsInfo` as the reference base answered them.
fn catalog_rows() -> Vec<ExtensionCatalogRow> {
    std::fs::read_to_string(fixtures().join("demo_store/extensions.tsv"))
        .unwrap()
        .lines()
        .map(|line| {
            let [identity, order, name, scope, distributed, info] =
                line.split('\t').collect::<Vec<_>>()[..]
            else {
                panic!("{line}");
            };
            ExtensionCatalogRow {
                identity: hex(identity),
                order: order.parse().unwrap(),
                name: name.to_owned(),
                info: hex(info),
                scope: Some(scope.parse().unwrap()),
                used_in_distributed_infobase: Some(distributed.parse().unwrap()),
            }
        })
        .collect()
}

/// A source answering the demo base's extension store, row for row.
fn demo() -> FakeSource {
    let mut source = FakeSource::new();
    let mut store = std::fs::read_dir(fixtures().join("demo_store"))
        .unwrap()
        .filter_map(|entry| {
            let path = entry.unwrap().path();
            let key = path
                .file_name()?
                .to_str()?
                .strip_suffix(".deflate")?
                .to_owned();
            Some((key, 0, std::fs::read(&path).unwrap()))
        })
        .collect::<Vec<_>>();
    store.sort();
    source.store_rows = store;
    source.extensions = catalog_rows();
    source
}

fn read(mut source: FakeSource) -> Vec<AcquiredExtension> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime
        .block_on(acquire_configuration(&mut source, &mut NoProgress))
        .unwrap()
        .extensions
}

fn guid(text: &str) -> Guid {
    Guid::from_str(text).unwrap()
}

#[test]
fn identity_order_and_activity_of_an_applied_and_a_not_applied_extension() {
    let extensions = read(demo());
    assert_eq!(extensions.len(), 2);
    let (first, second) = (&extensions[0], &extensions[1]);

    assert_eq!(first.name, "Расширение1");
    assert_eq!(first.identity, "90dc02d78b8d8e9911f1a8890af29d86");
    assert_eq!(first.order, 1);
    assert_eq!(first.active, Some(true));

    assert_eq!(second.name, "Расширение2");
    assert_eq!(second.identity, "949002d78b8d8e9911f1b656016e4730");
    assert_eq!(second.order, 2);
    assert_eq!(second.active, Some(false));
    // Not applied, but answered whole: skipping it is the consumer's call.
    assert!(second.catalog.is_some());
    assert!(
        second
            .objects
            .as_ref()
            .is_some_and(|objects| !objects.is_empty())
    );
}

#[test]
fn what_the_extensions_table_records() {
    for extension in read(demo()) {
        assert_eq!(extension.scope, Some(1), "{}", extension.name);
        assert_eq!(
            extension.used_in_distributed_infobase,
            Some(false),
            "{}",
            extension.name
        );
        // Not measured: never guessed.
        assert_eq!(extension.safe_mode, Some(true), "{}", extension.name);
        assert_eq!(extension.security_profile_name.as_deref(), Some(""));
        assert_eq!(extension.scope_kind, Some(crate::ExtensionScope::InfoBase));
        assert!(
            extension
                .hash_sum
                .as_ref()
                .is_some_and(|hash| hash.len() == 28)
        );
    }
}

#[test]
fn the_content_hash_is_the_key_of_the_root() {
    let extensions = read(demo());
    assert_eq!(
        extensions[0].content_hash,
        "89c0c813d3b9d05652ec8cef91e2ff6f1313f3e0"
    );
    assert_eq!(
        extensions[1].content_hash,
        "fc10f1ef43b842cabac33ca47292d1fe77f752bf"
    );
}

#[test]
fn a_version_is_answered_only_when_the_record_carries_one() {
    let extensions = read(demo());
    assert_eq!(extensions[0].version, None);

    // The tail of a `buh` record, which carries `2.4`, behind the root key
    // of `Расширение1`.
    let mut source = demo();
    let versioned = std::fs::read(fixtures().join("extension_info_versioned.bin")).unwrap();
    let mut info = source.extensions[0].info[..24].to_vec();
    info.extend_from_slice(&versioned[24..]);
    source.extensions[0].info = info;
    let extensions = read(source);
    assert_eq!(extensions[0].version.as_deref(), Some("2.4"));
    assert_eq!(extensions[0].active, Some(true));
}

#[test]
fn an_older_base_answers_scope_and_distribution_unknown() {
    let mut source = demo();
    for row in &mut source.extensions {
        row.scope = None;
        row.used_in_distributed_infobase = None;
    }
    for extension in read(source) {
        assert_eq!(extension.scope, None);
        assert_eq!(extension.used_in_distributed_infobase, None);
        assert!(extension.catalog.is_some(), "everything else is answered");
    }
}

#[test]
fn a_value_outside_the_measured_range_is_unknown_not_coerced() {
    let mut source = demo();
    source.extensions[0].scope = Some(-1);
    source.extensions[0].used_in_distributed_infobase = Some(2);
    source.extensions[1].used_in_distributed_infobase = Some(1);
    let extensions = read(source);
    assert_eq!(extensions[0].scope, None);
    assert_eq!(extensions[0].used_in_distributed_infobase, None);
    assert_eq!(extensions[1].used_in_distributed_infobase, Some(true));
}

#[test]
fn the_catalog_lists_the_classes_and_objects() {
    let extensions = read(demo());
    let catalog = extensions[0].catalog.as_ref().expect("a measured shape");
    assert_eq!(
        catalog.configuration,
        guid("182ffc1d-7701-4067-a547-c7b78532e40d")
    );
    let catalogs = catalog
        .classes
        .iter()
        .find(|entry| entry.class == guid(CATALOGS))
        .unwrap();
    assert_eq!(
        catalogs.objects,
        [
            guid("e1945025-1979-4ad7-8f06-3cf53c99530a"),
            guid("b7608864-b9ec-42d9-abb2-e7d7b0bdd348"),
        ]
    );
}

#[test]
fn an_adopted_object_links_to_the_base_through_its_own_header() {
    let extensions = read(demo());
    let objects = extensions[0].objects.as_ref().expect("a complete list");
    let find = |name: &str| {
        objects
            .iter()
            .find(|entry| entry.object.name == name)
            .unwrap_or_else(|| panic!("no header names {name}"))
    };

    let adopted = find("СтавкиНДС");
    assert_eq!(adopted.resource, "e1945025-1979-4ad7-8f06-3cf53c99530a");
    assert_eq!(adopted.object.belonging, Some(ObjectBelonging::Adopted));
    assert_eq!(
        adopted.object.extends,
        Some(guid("6a386de9-b78e-47cb-a7e8-864b52022b24"))
    );
    let attribute = find("Ставка");
    assert_eq!(attribute.resource, adopted.resource);
    assert_eq!(
        attribute.object.extends,
        Some(guid("9a871782-6f75-4078-a8f2-a33d9dcb8c7f"))
    );

    let own = find("Расш1_Справочник1");
    assert_eq!(own.object.belonging, Some(ObjectBelonging::Own));
    assert_eq!(own.object.extends, None);

    // A link names a main-configuration object, never a resource of the
    // extension store.
    for entry in objects {
        if let Some(extends) = &entry.object.extends {
            assert!(
                extensions[0]
                    .resources
                    .iter()
                    .all(|resource| !resource.file_name.starts_with(extends.as_str())),
                "{extends}"
            );
        }
    }
}

#[test]
fn a_configuration_root_not_in_the_measured_shape_answers_no_catalog() {
    let mut source = demo();
    // A stored DEFLATE block holding a root whose one entry counts two
    // objects and lists one.
    let malformed = b"{2,{182ffc1d-7701-4067-a547-c7b78532e40d},1,{cf4abea6-37b2-11d4-940f-008048da11f9,{1,{0},1,{cf4abea6-37b2-11d4-940f-008048da11f9,2,e1945025-1979-4ad7-8f06-3cf53c99530a}}}}";
    replace(&mut source, CONFIGURATION_ROOT, stored_deflate(malformed));

    let extensions = read(source);
    assert_eq!(extensions[0].catalog, None, "never a partial catalog");
    assert_eq!(extensions[0].active, Some(true));
    // Every description still decodes, but with no catalog there is nothing
    // to check the headers against: they are not answered as complete.
    assert_eq!(extensions[0].objects, None);
    assert!(extensions[1].catalog.is_some());
    assert!(extensions[1].objects.is_some());
}

/// The key of `СтавкиНДС`, a description of `Расширение1`.
const ADOPTED_CATALOG: &str = "33b58e119708b04f9eaa8a97133c443cefa7a3d3";
/// The key of `Расш1_Справочник1`, declared by the catalog of `Расширение1`.
const OWN_CATALOG: &str = "3a5bbff87a72dcc28135e9302e24c0006d23389f";
/// The key of `c4478a85-….0`, the rights of the role of `Расширение1`: a
/// resource whose name carries a suffix, not a description.
const ROLE_RIGHTS: &str = "429f01e1cccd745e0e5aca0878065f92e8f18580";

/// A raw-DEFLATE stream holding `data` in one stored block.
fn stored_deflate(data: &[u8]) -> Vec<u8> {
    let length = u16::try_from(data.len()).unwrap();
    let mut block = vec![0x01];
    block.extend_from_slice(&length.to_le_bytes());
    block.extend_from_slice(&(!length).to_le_bytes());
    block.extend_from_slice(data);
    block
}

/// Swaps the content the store holds under `key`: the store is
/// content-addressed, so the root index keeps naming the key.
fn replace(source: &mut FakeSource, key: &str, content: Vec<u8>) {
    let row = source
        .store_rows
        .iter_mut()
        .find(|(name, _, _)| name == key)
        .unwrap();
    row.2 = content;
}

#[test]
fn the_measured_fixture_answers_every_header() {
    let extensions = read(demo());
    let names = |index: usize| {
        let mut names = extensions[index]
            .objects
            .as_ref()
            .expect("complete")
            .iter()
            .map(|entry| entry.object.name.clone())
            .collect::<Vec<_>>();
        names.sort();
        names
    };
    assert_eq!(
        names(0),
        [
            "Расш1_ОсновнаяРоль",
            "Расш1_Реквизит1",
            "Расш1_Реквизит2",
            "Расш1_Реквизит3",
            "Расш1_Справочник1",
            "Расширение1",
            "Русский",
            "Ставка",
            "СтавкиНДС",
        ]
    );
    assert_eq!(names(1), ["Расш2_ОсновнаяРоль", "Расширение2", "Русский"]);
}

#[test]
fn one_damaged_description_answers_no_headers() {
    for damaged in [
        // Does not inflate.
        b"not a deflate stream".to_vec(),
        // Inflates, but is not the brace serialization.
        stored_deflate(b"{1,{unterminated"),
    ] {
        let mut source = demo();
        replace(&mut source, ADOPTED_CATALOG, damaged);
        let extensions = read(source);
        assert_eq!(extensions[0].objects, None, "never a shorter list");
        // The catalog, and the other extension, are unaffected.
        assert!(extensions[0].catalog.is_some());
        assert!(extensions[1].objects.is_some());
    }
}

#[test]
fn a_declared_object_without_its_header_answers_no_headers() {
    let mut source = demo();
    // The description parses, but carries no header for the catalog the
    // configuration declares.
    replace(&mut source, OWN_CATALOG, stored_deflate(b"{1,{0},0}"));
    let extensions = read(source);
    assert!(extensions[0].catalog.is_some());
    assert_eq!(extensions[0].objects, None);
}

#[test]
fn a_damaged_resource_that_is_no_description_leaves_the_list_complete() {
    let complete = read(demo())[0].objects.clone();
    let mut source = demo();
    replace(&mut source, ROLE_RIGHTS, b"not a deflate stream".to_vec());
    let extensions = read(source);
    assert!(complete.is_some());
    assert_eq!(extensions[0].objects, complete);
}

#[test]
fn unmeasured_scope_codes_stay_raw_and_have_no_typed_mapping() {
    let mut source = demo();
    source.extensions[0].scope = Some(42);
    let extensions = read(source);
    assert_eq!(extensions[0].scope, Some(42));
    assert_eq!(extensions[0].scope_kind, None);
}

#[test]
fn an_unknown_runtime_property_header_answers_unknown_properties() {
    let mut source = demo();
    source.extensions[0].info[35] = 0x83;
    let extensions = read(source);
    assert_eq!(extensions[0].safe_mode, None);
    assert_eq!(extensions[0].security_profile_name, None);
    assert_eq!(extensions[0].hash_sum, None);
    assert_eq!(extensions[1].safe_mode, Some(true));
}
