//! Expansion of restriction texts with the БСП templates of БП 3.0 under
//! `tests/fixtures/access`, and the combination of roles.

use std::path::PathBuf;
use std::str::FromStr;

use open_sdbl::access::{
    Access, RestrictionError, RestrictionScope, expand_restriction, read_access,
};
use open_sdbl::metadata::{Guid, Right, RoleRights, parse_role_rights};
use open_sdbl::query::{ParameterValue, QueryParameter, SessionParameters};

fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/access")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// `ЧтениеЭлектронныхДокументов`: reads with a restriction.
fn reading_role() -> RoleRights {
    parse_role_rights(&fixture(
        "buh_feadebe9-a90e-48b0-a89f-e1f5d4e23041.0.deflate",
    ))
    .unwrap()
}

/// `ПолныеПрава`: reads without one.
fn full_role() -> RoleRights {
    parse_role_rights(&fixture(
        "buh_e5c73637-e8d6-47e0-9c15-2fa1802ee5b0.0.deflate",
    ))
    .unwrap()
}

/// The first restricted object of the reading role, a catalog of
/// attached files.
fn restricted_object() -> Guid {
    Guid::from_str("d04d020d-c006-49a9-a8fe-788954f09f8d").unwrap()
}

const TABLE: &str = "Справочник.КвитанцииДТСПрисоединенныеФайлы";

fn session(values: &[(&str, ParameterValue)]) -> SessionParameters {
    let mut session = SessionParameters::new();
    for (name, value) in values {
        session.set(QueryParameter::new(*name, value.clone()));
    }
    session
}

fn text(value: &str) -> ParameterValue {
    ParameterValue::String(value.to_owned())
}

fn universal(versions: &str) -> SessionParameters {
    session(&[
        (
            "ОграничениеДоступаНаУровнеЗаписейУниверсально",
            ParameterValue::Boolean(true),
        ),
        ("СпискиСОтключеннымОграничениемЧтения", text("Все")),
        ("ВерсииШаблоновОграниченияДоступа", text(versions)),
    ])
}

fn read_condition(role: &RoleRights) -> String {
    role.object(&restricted_object())
        .unwrap()
        .right(&Right::Read)
        .unwrap()
        .restrictions[0]
        .condition
        .clone()
}

#[test]
fn expands_the_universal_branch_to_a_true_condition() {
    let role = reading_role();
    let session = universal(",ДляОбъекта9,");
    let scope = RestrictionScope {
        table_name: TABLE,
        right: &Right::Read,
        session: &session,
    };
    let expanded = expand_restriction(&read_condition(&role), &role.templates, &scope).unwrap();
    assert_eq!(expanded.alias, None);
    assert_eq!(expanded.condition, "ИСТИНА");
    assert_eq!(expanded.text(), "ТекущаяТаблица ГДЕ ИСТИНА");
}

#[test]
fn reports_an_outdated_template_as_its_message() {
    let role = reading_role();
    let session = universal("");
    let scope = RestrictionScope {
        table_name: TABLE,
        right: &Right::Read,
        session: &session,
    };
    let error = expand_restriction(&read_condition(&role), &role.templates, &scope).unwrap_err();
    match &error {
        RestrictionError::Message(message) => {
            assert!(
                message.starts_with("Ошибка: Требуется обновить шаблон"),
                "{message}"
            );
            // The current names are substituted into the message.
            assert!(
                message.contains(TABLE) && message.contains("Право: Чтение"),
                "{message}"
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn names_a_missing_session_parameter() {
    let role = reading_role();
    let session = session(&[(
        "ОграничениеДоступаНаУровнеЗаписейУниверсально",
        ParameterValue::Boolean(true),
    )]);
    let scope = RestrictionScope {
        table_name: TABLE,
        right: &Right::Read,
        session: &session,
    };
    let error = expand_restriction(&read_condition(&role), &role.templates, &scope).unwrap_err();
    assert_eq!(
        error,
        RestrictionError::MissingParameter("СпискиСОтключеннымОграничениемЧтения".to_owned())
    );
    assert!(
        error
            .to_string()
            .contains("СпискиСОтключеннымОграничениемЧтения")
    );
}

#[test]
fn expands_the_key_based_branch_into_a_correlated_condition() {
    let role = reading_role();
    // The universal template with the restriction on for this table: the
    // access keys branch.
    let session = session(&[
        (
            "ОграничениеДоступаНаУровнеЗаписейУниверсально",
            ParameterValue::Boolean(true),
        ),
        ("СпискиСОтключеннымОграничениемЧтения", text("")),
        ("ВерсииШаблоновОграниченияДоступа", text(",ДляОбъекта9,")),
        (
            "СпискиСОграничениемЧерезКлючиДоступаГруппДоступа",
            text(&format!("{TABLE}:ВладелецФайла;")),
        ),
        (
            "СпискиСОграничениемЧерезКлючиДоступаПользователей",
            text(""),
        ),
        ("ОбщиеПараметрыШаблоновОграниченияДоступа", text("")),
        ("СпискиСОграничениемПоПолям", text("")),
        (
            "ТекущийВнешнийПользователь",
            ParameterValue::Reference {
                object: open_sdbl::metadata::ObjectId::from(
                    &Guid::from_str("00000000-0000-0000-0000-000000000002").unwrap(),
                ),
                id: [0; 16],
            },
        ),
    ]);
    let scope = RestrictionScope {
        table_name: TABLE,
        right: &Right::Read,
        session: &session,
    };
    let expanded = expand_restriction(&read_condition(&role), &role.templates, &scope).unwrap();
    assert_eq!(expanded.alias, None);
    let condition = expanded
        .condition
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        condition.starts_with("ИСТИНА В ( ВЫБРАТЬ ПЕРВЫЕ 1 ИСТИНА ИЗ РегистрСведений.КлючиДоступаКОбъектам КАК КлючиДоступаКОбъектам ЛЕВОЕ СОЕДИНЕНИЕ РегистрСведений.КлючиДоступаНаборовГруппДоступа"),
        "{condition}"
    );
    // The object field named by the call reaches the correlated key.
    assert!(
        condition.contains("КлючиДоступаКОбъектам.Объект = ТекущаяТаблица.ВладелецФайла"),
        "{condition}"
    );
    assert!(!condition.contains('#'), "{condition}");
}

#[test]
fn a_freely_granting_role_lifts_the_restriction() {
    let reading = reading_role();
    let full = full_role();
    let session = universal(",ДляОбъекта9,");
    let scope = RestrictionScope {
        table_name: TABLE,
        right: &Right::Read,
        session: &session,
    };
    let object = restricted_object();
    assert_eq!(
        read_access(&[&reading, &full], &object, &Right::Read, &scope).unwrap(),
        Access::Unrestricted
    );
    assert_eq!(Access::Unrestricted.condition().unwrap(), None);

    let restricted = read_access(&[&reading], &object, &Right::Read, &scope).unwrap();
    let Access::Restricted(restrictions) = &restricted else {
        panic!("{restricted:?}");
    };
    assert_eq!(restrictions.len(), 1);
    assert_eq!(
        restricted.condition().unwrap().as_deref(),
        Some("ТекущаяТаблица ГДЕ ИСТИНА")
    );

    // Two restricting roles join with ИЛИ.
    let twice = read_access(&[&reading, &reading], &object, &Right::Read, &scope).unwrap();
    assert_eq!(
        twice.condition().unwrap().as_deref(),
        Some("ТекущаяТаблица ГДЕ (ИСТИНА) ИЛИ (ИСТИНА)")
    );

    // A right no role grants is denied; so is an object no role lists.
    assert_eq!(
        read_access(&[&reading], &object, &Right::Delete, &scope).unwrap(),
        Access::Denied
    );
    assert_eq!(
        Access::Denied.condition().unwrap().as_deref(),
        Some("ТекущаяТаблица ГДЕ ЛОЖЬ")
    );
    // An object no role lists: the full-rights role grants it by its
    // default, the reading role alone does not.
    let nowhere = Guid::from_str("00000000-0000-0000-0000-000000000001").unwrap();
    assert_eq!(
        read_access(&[&reading, &full], &nowhere, &Right::Read, &scope).unwrap(),
        Access::Unrestricted
    );
    assert_eq!(
        read_access(&[&reading], &nowhere, &Right::Read, &scope).unwrap(),
        Access::Denied
    );
}
