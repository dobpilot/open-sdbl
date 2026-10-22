use super::*;

const EMPTY_ROLE: &str = "2bc105dd-b388-436d-9410-a756e3415d30";
const ROLE: &str = "262144f7-02b6-4906-89fd-297cc72fe383";

fn empty_role_fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/extension/empty-role-native")
            .join(name),
    )
    .unwrap()
}

#[test]
fn native_empty_descriptor_matches_explicit_empty_rights() {
    let role = EMPTY_ROLE.parse().unwrap();
    let rights = parse_role_without_rights(&role, &empty_role_fixture("role.deflate")).unwrap();
    assert_eq!(
        rights,
        parse_role_rights(&empty_role_fixture("explicit-empty-rights.deflate")).unwrap()
    );
    assert!(
        parse_role_without_rights(&role, &empty_role_fixture("explicit-empty-role.deflate"))
            .is_none()
    );
    assert!(parse_role_without_rights(&role, b"not deflate").is_none());
    assert!(parse_role_without_rights(&role, &[]).is_none());
}

fn replace_value(value: &mut Value, path: &[usize], replacement: Value) {
    if let Some((first, rest)) = path.split_first() {
        let Value::List(items) = value else {
            panic!("путь в измеренном дескрипторе должен вести в список");
        };
        replace_value(&mut items[*first], rest, replacement);
    } else {
        *value = replacement;
    }
}

#[test]
fn unknown_or_incomplete_no_rights_profiles_stay_unreadable() {
    let role = EMPTY_ROLE.parse::<Guid>().unwrap();
    let original = parse_serialized(&empty_role_fixture("role.serialized")).unwrap();
    assert!(role_descriptor_has_no_rights(&role, &original));
    for (path, replacement) in [
        (vec![0], Value::Atom("2".into())),
        (vec![2], Value::Atom("1".into())),
        (vec![1, 0], Value::Atom("7".into())),
        (vec![1, 2], Value::Atom("1".into())),
        (vec![1, 3], Value::Atom("1".into())),
        (vec![1, 4], Value::Atom("1".into())),
        (vec![1, 1, 0], Value::Atom("4".into())),
        (vec![1, 1, 1, 0], Value::Atom("0".into())),
        (vec![1, 1, 1, 1], Value::Atom("1".into())),
        (vec![1, 1, 1, 2], Value::Atom(Guid::NIL.into())),
        (vec![1, 1, 2], Value::String(String::new())),
        (vec![1, 1, 2], Value::Atom("not a quoted name".into())),
        (vec![1, 1, 3, 0], Value::Atom("2".into())),
        (vec![1, 1, 3, 1], Value::String(String::new())),
        (vec![1, 1, 4], Value::Null),
        (vec![1, 1, 5], Value::Atom("1".into())),
        (vec![1, 1, 6], Value::Atom("1".into())),
        (vec![1, 1, 7], Value::Atom(EMPTY_ROLE.into())),
        (vec![1, 1, 8], Value::Atom("1".into())),
        (vec![1, 1], Value::List(Vec::new())),
    ] {
        let mut value = original.clone();
        replace_value(&mut value, &path, replacement);
        assert!(
            !role_descriptor_has_no_rights(&role, &value),
            "неизмерённое изменение по пути {path:?}"
        );
    }
    let mut truncated = original.clone();
    let Value::List(items) = &mut truncated else {
        unreachable!();
    };
    items.pop();
    assert!(!role_descriptor_has_no_rights(&role, &truncated));
    let Value::List(items) = &mut truncated else {
        unreachable!();
    };
    items.push(Value::Atom("0".into()));
    items.push(Value::Atom("0".into()));
    assert!(!role_descriptor_has_no_rights(&role, &truncated));
    assert!(!role_descriptor_has_no_rights(
        &ROLE.parse::<Guid>().unwrap(),
        &original
    ));
    assert!(!role_descriptor_has_no_rights(
        &Guid::NIL.parse::<Guid>().unwrap(),
        &original
    ));
}
