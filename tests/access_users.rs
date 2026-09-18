//! Users of an information base: the `v8users` rows of the УНФ demo under
//! `tests/fixtures/access`, decoded and named through the role catalog.

use std::path::PathBuf;
use std::str::FromStr;

use open_sdbl::metadata::{
    Guid, InfoBaseUser, MsSqlMetadataQueries, PostgresMetadataQueries, RoleCatalog, UserRow,
    decode_user_data, parse_config_descriptors,
};

fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/access")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The rows of `unf_v8users.tsv`: name, description, OS login, RolesID,
/// show, EAuth, AdmRole, `Data` as hex.
fn users() -> Vec<InfoBaseUser> {
    let text = String::from_utf8(fixture("unf_v8users.tsv")).unwrap();
    text.lines()
        .map(|line| {
            let columns = line.split('|').collect::<Vec<_>>();
            let flag = |index: usize| matches!(columns[index], "t" | "true" | "1");
            let data = (0..columns[7].len())
                .step_by(2)
                .map(|at| u8::from_str_radix(&columns[7][at..at + 2], 16).unwrap())
                .collect::<Vec<_>>();
            InfoBaseUser::new(
                UserRow {
                    name: columns[0],
                    description: columns[1],
                    os_name: columns[2],
                    show_in_list: flag(4),
                    standard_authentication: flag(5),
                    administrative: flag(6),
                },
                &data,
            )
            .unwrap()
        })
        .collect()
}

fn catalog() -> RoleCatalog {
    let guids = [
        "76702e9e-fa7a-4b98-befa-f9b37db2dae0",
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
    RoleCatalog::from_descriptors(&guids, &descriptors)
}

#[test]
fn decodes_the_demo_users_with_their_roles() {
    let users = users();
    assert_eq!(users.len(), 4);
    let catalog = catalog();

    let director = users
        .iter()
        .find(|user| user.name == "Абдулов (директор)")
        .unwrap();
    assert_eq!(director.description, "Абдулов Юрий Владимирович");
    assert_eq!(director.data.name, "Абдулов (директор)");
    assert_eq!(director.data.full_name, "Абдулов Юрий Владимирович");
    assert!(director.show_in_list && director.standard_authentication && director.administrative);
    assert_eq!(director.data.roles.len(), 3);
    let mut names = director.role_names(&catalog);
    names.sort();
    assert_eq!(
        names,
        [
            "АдминистраторСистемы",
            "ИнтерактивноеОткрытиеВнешнихОтчетовИОбработок",
            "ПолныеПрава",
        ]
    );

    let bot = users.iter().find(|user| user.name == "EDIEvents").unwrap();
    assert_eq!(bot.description, "Бот 1С-ЭДО");
    assert!(!bot.show_in_list);
    assert_eq!(bot.data.roles.len(), 1);

    // The nameless user has no roles.
    let nameless = users.iter().find(|user| user.name.is_empty()).unwrap();
    assert!(nameless.data.roles.is_empty());
    assert!(!nameless.data.id.is_nil());
}

#[test]
fn refuses_a_malformed_blob() {
    assert!(decode_user_data(&[]).is_err());
    assert!(decode_user_data(&[5, 1, 2]).is_err());
    // A well-formed key over content that is not a record.
    assert!(decode_user_data(&[1, 0x55, 0x55, 0x55]).is_err());
}

#[test]
fn provides_the_users_statements() {
    assert!(PostgresMetadataQueries::USERS.starts_with("SELECT name::text, descr::text"));
    assert!(PostgresMetadataQueries::USERS.ends_with("FROM v8users ORDER BY name"));
    assert!(MsSqlMetadataQueries::USERS.contains("FROM [dbo].[v8users] ORDER BY [Name]"));
}
