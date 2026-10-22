use super::*;

const RUSSIAN: &[u8] =
    include_bytes!("../../tests/fixtures/user-interface-language/LangRussian.data");
const ENGLISH: &[u8] =
    include_bytes!("../../tests/fixtures/user-interface-language/LangEnglish.data");
const DEFAULT: &[u8] =
    include_bytes!("../../tests/fixtures/user-interface-language/LangDefault.data");
const ADMIN: &[u8] = include_bytes!("../../tests/fixtures/user-interface-language/LangAdmin.data");

pub(super) fn fields(blob: &[u8]) -> Vec<Value> {
    let length = usize::from(blob[0]);
    let decoded: Vec<u8> = blob[1 + length..]
        .iter()
        .zip(blob[1..1 + length].iter().cycle())
        .map(|(a, b)| a ^ b)
        .collect();
    let Value::List(fields) = parse_serialized(&decoded).unwrap() else {
        panic!("expected a user list")
    };
    fields
}

#[test]
fn native_users_preserve_selected_and_known_default_language_without_mutation() {
    for (bytes, name, expected) in [
        (
            RUSSIAN,
            "LangRussian",
            "855375b9-327b-53fa-b11c-6454c2c7a1a6",
        ),
        (
            ENGLISH,
            "LangEnglish",
            "220d033d-4a5f-45ed-926b-af14b6cf30bc",
        ),
        (
            DEFAULT,
            "LangDefault",
            "00000000-0000-0000-0000-000000000000",
        ),
        (ADMIN, "LangAdmin", "220d033d-4a5f-45ed-926b-af14b6cf30bc"),
    ] {
        let original = bytes.to_vec();
        let data = decode_user_data(bytes).unwrap();
        assert_eq!(
            data.interface_language,
            Some(Guid::from_str(expected).unwrap())
        );
        assert_eq!(data.name, name);
        assert_eq!(data.full_name, format!("Native {name}"));
        let user = InfoBaseUser::new(
            UserRow {
                name,
                description: "own user",
                os_name: "",
                email: "",
                show_in_list: true,
                standard_authentication: true,
                administrative: false,
            },
            bytes,
        )
        .unwrap();
        assert_eq!(user.data, data);
        assert!(user.can_authenticate());
        assert_eq!(bytes, original);
    }
}

#[test]
fn unsupported_truncated_and_wrong_typed_languages_remain_unknown() {
    let original = fields(RUSSIAN);
    for end in 0..38 {
        assert_eq!(decode_interface_language(&original[..end]), None);
    }
    for replacement in [
        Value::Null,
        Value::List(vec![]),
        Value::Atom("bad-guid".into()),
        Value::String("855375b9-327b-53fa-b11c-6454c2c7a1a6".into()),
    ] {
        let mut changed = original.clone();
        changed[6] = replacement;
        assert_eq!(decode_interface_language(&changed), None);
    }
    let mut extended = original.clone();
    extended.push(Value::Atom("0".into()));
    assert_eq!(decode_interface_language(&extended), None);
    let mut inconsistent = original;
    inconsistent[30] = Value::Atom("0".into());
    assert_eq!(decode_interface_language(&inconsistent), None);
}

#[test]
fn replacing_only_language_preserves_the_other_decoded_fields() {
    let bytes = RUSSIAN;
    let length = usize::from(bytes[0]);
    let decoded: Vec<u8> = bytes[1 + length..]
        .iter()
        .zip(bytes[1..1 + length].iter().cycle())
        .map(|(a, b)| a ^ b)
        .collect();
    let text = String::from_utf8(decoded).unwrap();
    let old = decode_user_data(bytes).unwrap();
    for language in [
        "220d033d-4a5f-45ed-926b-af14b6cf30bc",
        "00000000-0000-0000-0000-000000000000",
        "invalid-language",
    ] {
        let changed = text.replacen("855375b9-327b-53fa-b11c-6454c2c7a1a6", language, 1);
        let mut encoded = vec![1, 0x5a];
        encoded.extend(changed.bytes().map(|byte| byte ^ 0x5a));
        let mut user = decode_user_data(&encoded).unwrap();
        assert_eq!(user.interface_language, Guid::from_str(language).ok());
        if language == "invalid-language" {
            assert_eq!(user.data_separation, None);
            user.data_separation = old.data_separation.clone();
        }
        user.interface_language = old.interface_language.clone();
        assert_eq!(user, old);
    }
}
