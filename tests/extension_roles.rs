//! The content-addressed store of the configuration extensions: the root
//! key of an extension, the index its root carries, and the rights of a
//! role the extension declares. The fixtures are the resources of
//! `_ДемоРасширение` of the «1С:Документооборот» demo base.

use std::path::PathBuf;
use std::str::FromStr;

use open_sdbl::metadata::{
    Guid, MsSqlMetadataQueries, PostgresMetadataQueries, Right, StorageLayout, extension_root_key,
    inflate_raw_deflate, parse_config_descriptors, parse_extension_index, parse_role_rights,
    parse_serialized_sequence,
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
    assert!(PostgresMetadataQueries::EXTENSIONS_PROBE.contains("_extensionsinfo"));
    assert!(MsSqlMetadataQueries::EXTENSIONS_PROBE.contains("_ExtensionsInfo"));
}
