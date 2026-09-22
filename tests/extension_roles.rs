//! The content-addressed store of the configuration extensions: the root
//! key of an extension, the index its root carries, and the rights of a
//! role the extension declares. The fixtures are the resources of
//! `_ДемоРасширение` of the «1С:Документооборот» demo base.

use std::path::PathBuf;
use std::str::FromStr;

use open_sdbl::metadata::{
    Guid, MsSqlMetadataQueries, PostgresMetadataQueries, Right, StorageLayout, extension_root_key,
    inflate_raw_deflate, parse_config_descriptors, parse_extension_index, parse_extension_info,
    parse_role_rights, parse_serialized_sequence,
};

const ROOT: &str = "344e701a5292613d188f54a0461ba28cdc4e64a0";
const RIGHTS: &str = "335a19b81dcf015fa848dc376306a344b48e2b38";
const DESCRIPTOR: &str = "c401178cde58a03a4d921cff86190ac295496d7c";
const ROLE: &str = "262144f7-02b6-4906-89fd-297cc72fe383";

fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/extension")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn inflated(name: &str) -> Vec<u8> {
    inflate_raw_deflate(&fixture(name)).unwrap()
}

#[test]
fn reads_the_root_key_of_an_extension() {
    let info = fixture("extension_info.bin");
    let key = extension_root_key(&info).unwrap();
    assert_eq!(key.as_hex(), ROOT);
    assert_eq!(key.as_bytes().len(), 20);
    // A record shorter than the marker and the key is none.
    assert!(extension_root_key(&info[..16]).is_none());
    assert!(extension_root_key(&[]).is_none());
}

#[test]
fn reads_the_record_of_an_extension() {
    let info = fixture("extension_info.bin");
    let record = parse_extension_info(&info).unwrap();
    assert_eq!(record.root_key.as_hex(), ROOT);
    assert_eq!(record.version.as_deref(), Some("1.0.1.14"));
    let synonym = record.synonym.as_deref().unwrap();
    assert!(synonym.contains("Расширение"), "{synonym}");
    assert_eq!(record.active, Some(true), "the demo extension is applied");

    // A record shorter than the marker and the key answers none; one that
    // stops right after the key still answers the key.
    assert!(parse_extension_info(&info[..16]).is_none());
    let bare = parse_extension_info(&info[..24]).unwrap();
    assert_eq!(bare.root_key.as_hex(), ROOT);
    assert!(bare.synonym.is_none() && bare.version.is_none());
    assert_eq!(bare.active, None, "no record, no flag, no guess");
}

/// A record the decoder cannot follow must never yield an activity: a
/// byte it did not reach as a tag is payload, and reading payload as the
/// flag would report an extension the base applies as inactive.
#[test]
fn never_guesses_the_activity_of_a_record_it_could_not_follow() {
    let record = |tail: &[u8]| {
        let mut blob = vec![0x43, 0xc2, 0x9a, 0x14];
        blob.extend_from_slice(&[0x11_u8; 20]);
        blob.extend_from_slice(tail);
        parse_extension_info(&blob).expect("the key is always answered")
    };

    // A tag of unknown length: its payload must not be mistaken for the
    // flag, with or without the terminator the real records end in.
    assert_eq!(record(&[0x98, 0x02, 0x81, 0x82]).active, None);
    assert_eq!(record(&[0x98, 0x02, 0x81, 0x82, 0x20]).active, None);
    // A byte string whose payload happens to sit where the flag sits.
    assert_eq!(record(&[0x9a, 0x03, 0xaa, 0x81, 0xcc, 0x20]).active, None);
    // A field the decoder knows but that ends early: the key survives.
    let truncated = record(&[0x97]);
    assert_eq!(truncated.active, None);
    assert_eq!(truncated.root_key.as_hex(), "11".repeat(20));
    assert_eq!(record(&[0x97, 0x40, 0x00]).active, None);
    // A record the decoder does follow to its terminator answers both.
    assert_eq!(record(&[0xa2, 0x81, 0x82, 0x82, 0x20]).active, Some(true));
    assert_eq!(record(&[0xa2, 0x81, 0x81, 0x82, 0x20]).active, Some(false));
    // An unmeasured value where the flag sits is no answer either.
    assert_eq!(record(&[0xa2, 0x81, 0xa1, 0x82, 0x20]).active, None);
}

/// The two extensions of the PostgreSQL reference base, one the platform
/// applies and one it does not. Everything else about them is equal, so
/// the difference between the records is the applicability flag alone.
#[test]
fn reads_whether_the_base_applies_an_extension() {
    let applied = fixture("extension_info_applied.bin");
    let not_applied = fixture("extension_info_not_applied.bin");
    assert_eq!(applied.len(), not_applied.len());

    let differing = applied
        .iter()
        .zip(&not_applied)
        .enumerate()
        .filter(|(_, (left, right))| left != right)
        .map(|(offset, _)| offset)
        .collect::<Vec<_>>();
    // The root key, the one character of the synonym naming them apart,
    // and the flag: three bytes from the end of the record.
    assert_eq!(differing.last(), Some(&(applied.len() - 3)));
    assert_eq!(applied[applied.len() - 3], 0x82);
    assert_eq!(not_applied[not_applied.len() - 3], 0x81);

    let applied = parse_extension_info(&applied).unwrap();
    let not_applied = parse_extension_info(&not_applied).unwrap();
    assert_eq!(applied.active, Some(true));
    assert_eq!(not_applied.active, Some(false));
    assert_ne!(applied.root_key.as_hex(), not_applied.root_key.as_hex());
    // Neither carries a version, and both name themselves in the synonym.
    assert!(applied.version.is_none() && not_applied.version.is_none());
    assert!(applied.synonym.as_deref().unwrap().contains("асширение1"));
    assert!(
        not_applied
            .synonym
            .as_deref()
            .unwrap()
            .contains("асширение2")
    );
    // Flipping the one byte turns one record into the other's answer,
    // which is what makes it the flag rather than a coincidence.
    let mut flipped = fixture("extension_info_applied.bin");
    let flag = flipped.len() - 3;
    flipped[flag] = 0x81;
    assert_eq!(parse_extension_info(&flipped).unwrap().active, Some(false));
}

#[test]
fn reads_the_resource_index_of_an_extension() {
    let root = inflated(&format!("root_{ROOT}.deflate"));
    let resources = parse_extension_index(&root).unwrap();
    assert_eq!(resources.len(), 169);

    let rights = resources
        .iter()
        .find(|resource| resource.name == format!("{ROLE}.0"))
        .expect("the rights resource of the role");
    assert_eq!(rights.key.as_hex(), RIGHTS);
    let descriptor = resources
        .iter()
        .find(|resource| resource.name == ROLE)
        .expect("the descriptor of the role");
    assert_eq!(descriptor.key.as_hex(), DESCRIPTOR);

    // A resource that lists nothing is refused.
    let error = parse_extension_index(b"{0}").unwrap_err();
    assert!(error.message().contains("lists no resources"), "{error}");
}

#[test]
fn reads_the_rights_and_the_name_of_an_extension_role() {
    let guid = Guid::from_str(ROLE).unwrap();
    let rights = parse_role_rights(&fixture(&format!("rights_{RIGHTS}.deflate"))).unwrap();
    assert!(!rights.objects.is_empty());
    // The role lists what it grants, as a role of the configuration does.
    assert!(
        rights
            .objects
            .iter()
            .flat_map(|object| &object.rights)
            .any(|right| right.allowed && right.right == Right::Read),
        "{:?}",
        rights.objects.first()
    );

    let descriptors =
        parse_config_descriptors(ROLE, &fixture(&format!("descriptor_{DESCRIPTOR}.deflate")))
            .unwrap();
    let descriptor = descriptors
        .iter()
        .find(|descriptor| descriptor.object_guid == guid)
        .expect("the descriptor names the role");
    assert_eq!(descriptor.name, "_ДемоБазовыеПрава");
    assert_eq!(
        descriptor
            .synonyms
            .first()
            .map(|synonym| synonym.text.as_str()),
        Some("Демо: Базовые права (из расширения)")
    );
}

#[test]
fn parses_a_resource_of_several_records() {
    let root = inflated(&format!("root_{ROOT}.deflate"));
    let records = parse_serialized_sequence(&root).unwrap();
    assert!(records.len() >= 3, "{}", records.len());
    // A single record parses as one.
    assert_eq!(parse_serialized_sequence(b"{1,2}").unwrap().len(), 1);
    assert!(parse_serialized_sequence(b"  ").is_err());
}

#[test]
fn builds_the_statements_of_the_extension_store() {
    let info = fixture("extension_info.bin");
    let key = extension_root_key(&info).unwrap();
    assert_eq!(
        PostgresMetadataQueries::extension_resource(&StorageLayout::MODERN, &key),
        format!(
            "SELECT binarydata FROM configcas WHERE rtrim(filename::text) = '{ROOT}' ORDER BY partno"
        )
    );
    assert_eq!(
        MsSqlMetadataQueries::extension_resource(&StorageLayout::MODERN, &key),
        format!(
            "SELECT [BinaryData] FROM [dbo].[ConfigCAS] WHERE CONVERT(nvarchar(128), [FileName]) = N'{ROOT}' ORDER BY [PartNo]"
        )
    );
    // A store without parts reads the single row.
    assert!(
        !PostgresMetadataQueries::extension_resource(&StorageLayout::LEGACY, &key)
            .contains("partno"),
        "the legacy store has no PartNo"
    );
    assert!(PostgresMetadataQueries::EXTENSIONS.contains("_extensionsinfo"));
    assert!(MsSqlMetadataQueries::EXTENSIONS.contains("_ExtensionsInfo"));
    // The statement answers the identity and the order beside the name.
    assert!(PostgresMetadataQueries::EXTENSIONS.contains("_idrref"));
    assert!(MsSqlMetadataQueries::EXTENSIONS.contains("[_IDRRef]"));
    assert!(PostgresMetadataQueries::EXTENSIONS_PROBE.contains("_extensionsinfo"));
    assert!(MsSqlMetadataQueries::EXTENSIONS_PROBE.contains("_ExtensionsInfo"));
}
