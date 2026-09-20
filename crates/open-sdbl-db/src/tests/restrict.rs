//! Tests of the `restrict` module.

use open_sdbl::query::RestrictionTarget;

use super::*;
use crate::enumeration_snapshot_support::enumeration_snapshot;

#[test]
fn parses_restriction_commands() {
    assert_eq!(
        parse_restriction_command("\\restrict Справочник.Номенклатура Организация = &Орг"),
        Some(RestrictionCommand::Set {
            name: "Справочник.Номенклатура",
            condition: "Организация = &Орг"
        })
    );
    assert_eq!(
        parse_restriction_command("\\restrict Документ.Реализация.Товары Сумма > 0"),
        Some(RestrictionCommand::Set {
            name: "Документ.Реализация.Товары",
            condition: "Сумма > 0"
        })
    );
    assert_eq!(
        parse_restriction_command("\\restrict"),
        Some(RestrictionCommand::List)
    );
    assert_eq!(
        parse_restriction_command("\\restrict Clear"),
        Some(RestrictionCommand::Clear)
    );
    assert_eq!(
        parse_restriction_command("\\restrict Номенклатура Код = 1"),
        Some(RestrictionCommand::Usage)
    );
    assert_eq!(
        parse_restriction_command("\\restrict Справочник.Номенклатура"),
        Some(RestrictionCommand::Usage)
    );
    assert_eq!(parse_restriction_command("\\set x 1"), None);
}

#[test]
fn stores_lists_and_filters_restrictions_by_request() {
    let snapshot = enumeration_snapshot();
    let object = ObjectId::from(
        &find_metadata_object(&snapshot, "Перечисление.бит_ВидыСтатусовОбъектов")
            .unwrap()
            .guid,
    );
    let mut store = RestrictionStore::new();
    let output = apply_restriction_command(
        &mut store,
        RestrictionCommand::Set {
            name: "Перечисление.бит_ВидыСтатусовОбъектов",
            condition: "Порядок > 0",
        },
        &snapshot,
    )
    .unwrap();
    assert_eq!(
        output,
        "Restriction of Перечисление.бит_ВидыСтатусовОбъектов set.\n"
    );
    apply_restriction_command(
        &mut store,
        RestrictionCommand::Set {
            name: "перечисление.бит_ВидыСтатусовОбъектов",
            condition: "Порядок > 1",
        },
        &snapshot,
    )
    .unwrap();
    apply_restriction_command(
        &mut store,
        RestrictionCommand::Set {
            name: "Перечисление.бит_ВидыСтатусовОбъектов.Строки",
            condition: "Сумма > 0",
        },
        &snapshot,
    )
    .unwrap();
    assert_eq!(
        store.listing(),
        "  перечисление.бит_ВидыСтатусовОбъектов         Порядок > 1\n  Перечисление.бит_ВидыСтатусовОбъектов.Строки  Сумма > 0\n"
    );

    let unknown = apply_restriction_command(
        &mut store,
        RestrictionCommand::Set {
            name: "Справочник.Нет",
            condition: "Код = 1",
        },
        &snapshot,
    )
    .unwrap_err();
    assert!(unknown.to_string().contains("was not found"));

    let request = RestrictionRequest {
        targets: vec![RestrictionTarget {
            object,
            table_part: Some("строки".to_owned()),
        }],
    };
    let restrictions = store.for_request(&request);
    assert_eq!(restrictions.len(), 1);
    assert_eq!(restrictions[0].condition(), "Сумма > 0");
    assert_eq!(restrictions[0].table_part_name(), Some("Строки"));
    assert!(store.for_request(&RestrictionRequest::default()).is_empty());

    assert_eq!(
        apply_restriction_command(&mut store, RestrictionCommand::Clear, &snapshot).unwrap(),
        "Restrictions cleared.\n"
    );
    assert_eq!(store.listing(), "No restrictions set.\n");
    assert!(apply_restriction_command(&mut store, RestrictionCommand::Usage, &snapshot).is_err());
}

#[test]
fn derived_restrictions_yield_to_typed_ones_and_are_forgotten() {
    let snapshot = enumeration_snapshot();
    let object = ObjectId::from(
        &find_metadata_object(&snapshot, "Перечисление.бит_ВидыСтатусовОбъектов")
            .unwrap()
            .guid,
    );
    let mut store = RestrictionStore::new();
    let derived = vec![(
        "Перечисление.бит_ВидыСтатусовОбъектов".to_owned(),
        object,
        "ИСТИНА".to_owned(),
    )];
    assert_eq!(store.derive(derived.clone()), 1);
    let listing = store.listing();
    assert!(
        listing.starts_with("* Перечисление.бит_ВидыСтатусовОбъектов  ИСТИНА\n"),
        "{listing}"
    );
    assert!(
        listing.ends_with("# * derived from the roles of the current user\n"),
        "{listing}"
    );

    // Deriving again replaces the derived restrictions, not the typed ones.
    apply_restriction_command(
        &mut store,
        RestrictionCommand::Set {
            name: "Перечисление.бит_ВидыСтатусовОбъектов",
            condition: "Порядок > 0",
        },
        &snapshot,
    )
    .unwrap();
    assert_eq!(store.derive(derived), 0);
    assert_eq!(
        store.listing(),
        "  Перечисление.бит_ВидыСтатусовОбъектов  Порядок > 0\n"
    );

    // A derived restriction is forgotten; the typed one stays.
    let mut store = RestrictionStore::new();
    store.derive(vec![(
        "Перечисление.бит_ВидыСтатусовОбъектов".to_owned(),
        object,
        "ИСТИНА".to_owned(),
    )]);
    store.forget_derived();
    assert_eq!(store.listing(), "No restrictions set.\n");
}
