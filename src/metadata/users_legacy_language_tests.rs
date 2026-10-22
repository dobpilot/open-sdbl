use super::language_tests::fields;
use super::*;

fn legacy(name: &str, layout: usize) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/user-interface-language/legacy/inputs")
            .join(format!("{name}.legacy-{layout}.data")),
    )
    .unwrap()
}

#[test]
fn native_legacy_choices_preserve_identity_names_and_roles() {
    for layout in [31, 37] {
        for name in ["LangRussian", "LangEnglish", "LangDefault"] {
            let bytes = legacy(name, layout);
            let current = std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/user-interface-language/legacy/inputs")
                    .join(format!("{name}.data")),
            )
            .unwrap();
            let mut expected = decode_user_data(&current).unwrap();
            let actual = decode_user_data(&bytes).unwrap();
            assert!(actual.interface_language.is_some());
            assert_eq!(actual.interface_language, expected.interface_language);
            expected.data_separation = actual.data_separation.clone();
            assert_eq!(actual, expected);
        }
    }
}

#[test]
fn malformed_legacy_structures_and_unmeasured_form_remain_unknown() {
    for layout in [31, 37] {
        let original = fields(&legacy("LangRussian", layout));
        for end in 0..layout {
            // Полная форма31 сама измерена; это не повреждённый хвост формы37.
            if end == 31 {
                continue;
            }
            assert_eq!(decode_interface_language(&original[..end]), None);
        }
        for replacement in [
            Value::Null,
            Value::List(vec![]),
            Value::String("855375b9-327b-53fa-b11c-6454c2c7a1a6".into()),
            Value::Atom("bad-guid".into()),
        ] {
            let mut changed = original.clone();
            changed[6] = replacement;
            assert_eq!(decode_interface_language(&changed), None);
        }
        for count in ["1", "2"] {
            let mut changed = original.clone();
            changed[30] = Value::Atom(count.into());
            assert_eq!(decode_interface_language(&changed), None);
        }
        let mut changed = original.clone();
        changed.push(Value::Atom("0".into()));
        assert_eq!(decode_interface_language(&changed), None);
        let mut changed = original;
        changed[17] = Value::Atom("1".into());
        assert_eq!(decode_interface_language(&changed), None);
    }
    let mut form32 = fields(&legacy("LangRussian", 31));
    form32[30] = Value::Atom("1".into());
    form32.push(Value::String(String::new()));
    assert!(decode_data_separation(&form32).is_some());
    assert_eq!(decode_interface_language(&form32), None);
}
