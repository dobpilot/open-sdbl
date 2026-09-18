//! Tests of the `access` module on the fixtures under `tests/fixtures/access`
//! of the library crate.

use std::path::PathBuf;
use std::str::FromStr;

use open_sdbl::metadata::parse_config_descriptors;

use super::*;

fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/access")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

const ROLE_GUIDS: [&str; 4] = [
    "76702e9e-fa7a-4b98-befa-f9b37db2dae0",
    "849f034e-85dc-4515-aae6-240c1e0d46d9",
    "e960b3eb-ad6f-4804-b318-acbcdc4b8f98",
    "6b6566ca-81d0-4376-93d8-290301f2b00f",
];

fn catalog() -> RoleCatalog {
    let guids = ROLE_GUIDS.map(|guid| Guid::from_str(guid).unwrap());
    let mut descriptors = Vec::new();
    for guid in &guids {
        descriptors.extend(
            parse_config_descriptors(guid.as_str(), &fixture(&format!("unf_{guid}.deflate")))
                .unwrap(),
        );
    }
    RoleCatalog::from_descriptors(&guids, &descriptors)
}

/// The `v8users` rows of the УНФ demo as the users statement answers them.
fn user_rows() -> QueryRows {
    let text = String::from_utf8(fixture("unf_v8users.tsv")).unwrap();
    text.lines()
        .map(|line| {
            let columns = line.split('|').collect::<Vec<_>>();
            let flag = |index: usize| {
                Cell::Number(if matches!(columns[index], "t" | "true" | "1") {
                    "1".to_owned()
                } else {
                    "0".to_owned()
                })
            };
            let data = (0..columns[7].len())
                .step_by(2)
                .map(|at| u8::from_str_radix(&columns[7][at..at + 2], 16).unwrap())
                .collect::<Vec<_>>();
            vec![
                Cell::Text(columns[0].to_owned()),
                Cell::Text(columns[1].to_owned()),
                Cell::Text(columns[2].to_owned()),
                flag(4),
                flag(5),
                flag(6),
                Cell::Bytes(data),
            ]
        })
        .collect()
}

#[test]
fn parses_access_commands() {
    assert_eq!(parse_access_command("\\users"), Some(AccessCommand::Users));
    assert_eq!(
        parse_access_command("\\user Абдулов (директор)"),
        Some(AccessCommand::User("Абдулов (директор)"))
    );
    assert_eq!(
        parse_access_command("\\roles"),
        Some(AccessCommand::Roles(None))
    );
    assert_eq!(
        parse_access_command("\\roles Полные"),
        Some(AccessCommand::Roles(Some("Полные")))
    );
    assert_eq!(
        parse_access_command("\\role ПолныеПрава Справочник.Организации"),
        Some(AccessCommand::Role {
            name: "ПолныеПрава",
            object: Some("Справочник.Организации")
        })
    );
    assert_eq!(
        parse_access_command("\\role Полные Права"),
        Some(AccessCommand::Role {
            name: "Полные Права",
            object: None
        })
    );
    assert_eq!(
        parse_access_command("\\rls Справочник.Организации Изменение"),
        Some(AccessCommand::Rls {
            object: "Справочник.Организации",
            right: Some("Изменение")
        })
    );
    assert_eq!(parse_access_command("\\as"), Some(AccessCommand::As(None)));
    assert_eq!(
        parse_access_command("\\as clear"),
        Some(AccessCommand::AsClear)
    );
    assert_eq!(
        parse_access_command("\\as Петрова (бухгалтер)"),
        Some(AccessCommand::As(Some("Петрова (бухгалтер)")))
    );
    assert!(matches!(
        parse_access_command("\\role"),
        Some(AccessCommand::Usage(_))
    ));
    assert_eq!(parse_access_command("\\restrict"), None);
    assert_eq!(parse_access_command("ВЫБРАТЬ 1"), None);
}

#[test]
fn decodes_users_from_rows_and_lists_them() {
    let users = users_from_rows(&user_rows()).unwrap();
    assert_eq!(users.len(), 4);
    let catalog = catalog();
    let listing = list_users(&users, &catalog);
    assert!(listing.starts_with("name\tdescription\tos login\tshow\tauth\tadmin\troles\n"));
    assert!(
        listing.contains("Абдулов (директор)\tАбдулов Юрий Владимирович\t\tyes\tyes\tyes\t3\n"),
        "{listing}"
    );
    assert!(listing.ends_with("# 4 users\n"));

    let director = users
        .iter()
        .find(|user| user.name == "Абдулов (директор)")
        .unwrap();
    let description = describe_user(director, &catalog);
    assert!(description.contains("roles (3):\n"), "{description}");
    assert!(description.contains("  АдминистраторСистемы\tАдминистратор системы\n"));
    assert!(description.contains("  ПолныеПрава\t"));
}

#[test]
fn lists_roles_with_a_filter() {
    let catalog = catalog();
    let all = list_roles(&catalog, None);
    assert!(all.ends_with("# 4 roles\n"), "{all}");
    let filtered = list_roles(&catalog, Some("полные"));
    assert!(filtered.starts_with("ПолныеПрава\t"), "{filtered}");
    assert!(filtered.ends_with("# 1 roles\n"));
}

#[test]
fn assembles_rights_resources_from_rows() {
    let guid = Guid::from_str(ROLE_GUIDS[0]).unwrap();
    let bytes = fixture(&format!("unf_{guid}.0.deflate"));
    let (head, tail) = bytes.split_at(bytes.len() / 2);
    let rows = vec![
        vec![
            Cell::Text(format!("{guid}.0")),
            Cell::Number("0".to_owned()),
            Cell::Bytes(head.to_vec()),
        ],
        vec![
            Cell::Text(format!("{guid}.0")),
            Cell::Number("1".to_owned()),
            Cell::Bytes(tail.to_vec()),
        ],
    ];
    let decoded = role_rights_from_rows(&rows).unwrap();
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].0, guid);
    assert!(!decoded[0].1.objects.is_empty());

    let odd = vec![vec![
        Cell::Text("root".to_owned()),
        Cell::Number("0".to_owned()),
        Cell::Bytes(Vec::new()),
    ]];
    assert!(role_rights_from_rows(&odd).is_err());
}

#[test]
fn describes_a_role_without_metadata_by_identifier() {
    let snapshot = crate::params::tests::enumeration_snapshot();
    let guid = Guid::from_str(ROLE_GUIDS[0]).unwrap();
    let rights = parse_role_rights(&fixture(&format!("unf_{guid}.0.deflate"))).unwrap();
    let catalog = catalog();
    let role = catalog.by_guid(&guid).unwrap();
    let text = describe_role(role, &rights, &snapshot, None).unwrap();
    assert!(text.starts_with("role: АдминистраторСистемы\t"), "{text}");
    assert!(text.contains("objects;"), "{text}");
    // An object the snapshot does not know is named by its identifier.
    assert!(text.contains("-"), "{text}");
    let unknown = describe_role(role, &rights, &snapshot, Some("Справочник.Нет"));
    assert!(unknown.is_err());
}
