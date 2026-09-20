//! Tests of the `access` command module.

use super::{AccessCommand, parse_access_command};

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
    assert_eq!(parse_access_command("\\rls"), Some(AccessCommand::RlsList));
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
