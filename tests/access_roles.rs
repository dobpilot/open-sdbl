//! Roles of a configuration: the rights resource, the right names, the
//! roles collection and the catalog, on resources of БП 3.0 and УНФ under
//! `tests/fixtures/access`.

use std::path::PathBuf;
use std::str::FromStr;

use open_sdbl::metadata::{
    DEFAULT_OUTPUT_LIMIT, Guid, MsSqlMetadataQueries, PostgresMetadataQueries, Right, RoleCatalog,
    StorageLayout, inflate_raw_deflate, parse_config_descriptors, parse_config_resource_bounded,
    parse_role_rights, roles_collection,
};

fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/access")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

const READING_ROLE: &str = "feadebe9-a90e-48b0-a89f-e1f5d4e23041";
const ADMINISTRATOR_ROLE: &str = "76702e9e-fa7a-4b98-befa-f9b37db2dae0";

#[test]
fn decodes_a_read_only_role_with_a_restriction_and_templates() {
    let rights = parse_role_rights(&fixture(&format!("buh_{READING_ROLE}.0.deflate"))).unwrap();
    assert!(!rights.set_for_new_objects);
    assert!(rights.set_for_attributes_by_default);
    assert!(!rights.independent_rights_of_child_objects);
    assert_eq!(rights.objects.len(), 61);

    let first = &rights.objects[0];
    assert_eq!(
        first.object.as_str(),
        "d04d020d-c006-49a9-a8fe-788954f09f8d"
    );
    assert!(first.members.is_empty());
    let granted = first
        .rights
        .iter()
        .filter(|right| right.allowed)
        .map(|right| right.right.clone())
        .collect::<Vec<_>>();
    assert_eq!(granted, [Right::Read, Right::View, Right::InputByString]);
    let read = first.right(&Right::Read).unwrap();
    assert_eq!(read.restrictions.len(), 1);
    assert!(
        read.restrictions[0]
            .condition
            .starts_with("#Если &ОграничениеДоступаНаУровнеЗаписейУниверсально #Тогда"),
        "{}",
        read.restrictions[0].condition
    );
    assert!(read.restrictions[0].fields.is_empty());
    assert!(first.right(&Right::View).unwrap().restrictions.is_empty());

    let names = rights
        .templates
        .iter()
        .map(|template| template.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            "ДляОбъекта",
            "ДляРегистра",
            "ПоЗначениям",
            "ПоЗначениямРасширенный"
        ]
    );
    let for_register = rights.template("ДляРегистра").unwrap();
    assert_eq!(
        for_register.parameters,
        ["Регистр", "Поле1", "Поле2", "Поле3", "Поле4", "Поле5"]
    );
    assert!(
        for_register.body.contains("#Регистр"),
        "{}",
        for_register.body
    );
    assert_eq!(
        rights.template("ПоЗначениям").unwrap().parameters,
        Vec::<String>::new()
    );
}

#[test]
fn keeps_refused_rights_and_members() {
    let rights =
        parse_role_rights(&fixture(&format!("unf_{ADMINISTRATOR_ROLE}.0.deflate"))).unwrap();
    let refused = rights
        .objects
        .iter()
        .flat_map(|object| &object.rights)
        .filter(|right| !right.allowed)
        .count();
    assert!(
        refused > 0,
        "the administrator role refuses interactive deletion"
    );
    // Every right of the administrator role is a named one.
    let unnamed = rights
        .objects
        .iter()
        .flat_map(|object| &object.rights)
        .filter(|right| matches!(right.right, Right::Other(_)))
        .count();
    assert_eq!(unnamed, 0);
    // Rights set on a member — an attribute or a command — carry its path.
    assert!(
        rights
            .objects
            .iter()
            .any(|object| !object.members.is_empty())
            || rights
                .objects
                .iter()
                .all(|object| object.members.is_empty())
    );
}

#[test]
fn names_rights_by_identifier_and_by_spelling() {
    let read = Guid::from_str("1c87578f-9e09-4ec0-a991-5629c87b1588").unwrap();
    assert_eq!(Right::from_guid(&read), Right::Read);
    assert_eq!(Right::Read.guid(), read);
    assert_eq!(Right::Read.name(), "Read");
    assert_eq!(Right::Read.russian_name(), "Чтение");
    assert_eq!(Right::parse("чтение"), Some(Right::Read));
    assert_eq!(Right::parse("view"), Some(Right::View));
    assert_eq!(Right::parse("ничего"), None);
    let unknown = Guid::from_str("eb29e198-c338-4a20-a253-be6fc3dd44d9").unwrap();
    assert_eq!(Right::from_guid(&unknown), Right::Other(unknown.clone()));
    assert_eq!(Right::Other(unknown.clone()).name(), unknown.as_str());
}

#[test]
fn refuses_a_resource_of_another_shape() {
    // A role descriptor is a bare-GUID resource, not a rights one.
    let error = parse_role_rights(&fixture(&format!("buh_{READING_ROLE}.deflate"))).unwrap_err();
    assert!(error.message().contains("rights resource"), "{error}");
}

#[test]
fn projects_the_roles_collection_of_the_root_resource() {
    let root = fixture("root_roles.deflate");
    let decoded = inflate_raw_deflate(&root).unwrap();
    let roles = roles_collection(&decoded);
    assert_eq!(roles.len(), 2);
    let parsed = parse_config_resource_bounded(
        "00000000-0000-0000-0000-000000000001",
        &root,
        DEFAULT_OUTPUT_LIMIT,
    )
    .unwrap();
    assert_eq!(parsed.roles, roles);
    // A resource without the collection lists none.
    let other = parse_config_resource_bounded(
        READING_ROLE,
        &fixture(&format!("buh_{READING_ROLE}.deflate")),
        DEFAULT_OUTPUT_LIMIT,
    )
    .unwrap();
    assert!(other.roles.is_empty());
}

#[test]
fn catalogs_roles_from_their_descriptors() {
    let guids = [
        ADMINISTRATOR_ROLE,
        "849f034e-85dc-4515-aae6-240c1e0d46d9",
        "e960b3eb-ad6f-4804-b318-acbcdc4b8f98",
        "6b6566ca-81d0-4376-93d8-290301f2b00f",
    ]
    .map(|guid| Guid::from_str(guid).unwrap());
    let mut descriptors = Vec::new();
    for guid in &guids {
        descriptors.extend(
            parse_config_descriptors(guid.as_str(), &fixture(&format!("unf_{guid}.deflate")))
                .unwrap(),
        );
    }
    let catalog = RoleCatalog::from_descriptors(&guids, &descriptors);
    assert_eq!(catalog.roles().len(), 4);
    let administrator = catalog.by_name("администраторсистемы").unwrap();
    assert_eq!(administrator.guid, guids[0]);
    assert_eq!(administrator.name, "АдминистраторСистемы");
    assert_eq!(
        administrator.synonym.as_deref(),
        Some("Администратор системы")
    );
    assert_eq!(
        catalog.by_guid(&guids[1]).unwrap().name,
        "ИнтерактивноеОткрытиеВнешнихОтчетовИОбработок"
    );
    assert!(catalog.by_name("Нет").is_none());
    // A role without a descriptor keeps its identifier as the name.
    let orphan = Guid::from_str("00000000-0000-0000-0000-000000000009").unwrap();
    let catalog = RoleCatalog::from_descriptors(std::slice::from_ref(&orphan), &descriptors);
    assert_eq!(catalog.roles()[0].name, orphan.as_str());
}

#[test]
fn builds_the_rights_acquisition_statements() {
    let roles = [READING_ROLE, ADMINISTRATOR_ROLE].map(|guid| Guid::from_str(guid).unwrap());
    let modern = PostgresMetadataQueries::role_rights(&StorageLayout::MODERN, &roles);
    assert_eq!(
        modern,
        format!(
            "SELECT rtrim(filename::text), partno, binarydata FROM config WHERE rtrim(filename::text) IN ('{READING_ROLE}.0', '{ADMINISTRATOR_ROLE}.0') ORDER BY filename, partno"
        )
    );
    let legacy = PostgresMetadataQueries::role_rights(&StorageLayout::LEGACY, &roles);
    assert!(legacy.contains("0::int") && legacy.ends_with("ORDER BY filename"));
    let mssql = MsSqlMetadataQueries::role_rights(&StorageLayout::MODERN, &roles[..1]);
    assert_eq!(
        mssql,
        format!(
            "SELECT CONVERT(nvarchar(128), [FileName]), [PartNo], [BinaryData] FROM [dbo].[Config] WHERE [FileName] IN (N'{READING_ROLE}.0') ORDER BY [FileName], [PartNo]"
        )
    );
    // No roles: a valid statement that reads nothing.
    let none = PostgresMetadataQueries::role_rights(&StorageLayout::MODERN, &[]);
    assert!(none.contains("IN ('none.0')"));
}

#[test]
fn rights_not_listed_follow_the_role_default() {
    let full = parse_role_rights(&fixture(
        "unf_e960b3eb-ad6f-4804-b318-acbcdc4b8f98.0.deflate",
    ))
    .unwrap();
    assert!(full.set_for_new_objects);
    // An object the full-rights role lists with refusals only.
    let listed = full
        .objects
        .iter()
        .find(|entry| entry.members.is_empty() && entry.rights.iter().all(|right| !right.allowed))
        .expect("an object with refusals only");
    assert!(full.grants(&listed.object, &Right::Read));
    assert!(!full.grants(&listed.object, &listed.rights[0].right));
    assert!(full.restrictions(&listed.object, &Right::Read).is_empty());
    // An object it does not list at all.
    let unlisted = Guid::from_str("00000000-0000-0000-0000-000000000001").unwrap();
    assert!(full.grants(&unlisted, &Right::Read));

    let reading = parse_role_rights(&fixture(&format!("buh_{READING_ROLE}.0.deflate"))).unwrap();
    assert!(!reading.set_for_new_objects);
    assert!(!reading.grants(&unlisted, &Right::Read));
    let object = Guid::from_str("d04d020d-c006-49a9-a8fe-788954f09f8d").unwrap();
    assert!(reading.grants(&object, &Right::Read));
    assert!(!reading.grants(&object, &Right::Delete));
    assert_eq!(reading.restrictions(&object, &Right::Read).len(), 1);
}
