use super::*;

#[test]
fn ordinary_database_rows_retain_selected_and_default_language() {
    for (name, expected) in [
        ("LangRussian", Some("855375b9-327b-53fa-b11c-6454c2c7a1a6")),
        ("LangEnglish", Some("220d033d-4a5f-45ed-926b-af14b6cf30bc")),
        ("LangDefault", Some("00000000-0000-0000-0000-000000000000")),
    ] {
        let bytes = std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/user-interface-language")
                .join(format!("{name}.data")),
        )
        .unwrap();
        let rows = vec![vec![
            Cell::Text(name.into()),
            Cell::Text("own user".into()),
            Cell::Text(String::new()),
            Cell::Number("1".into()),
            Cell::Number("1".into()),
            Cell::Number("0".into()),
            Cell::Bytes(bytes),
        ]];
        let users = users_from_rows(&rows).unwrap();
        let snapshot = crate::enumeration_snapshot_support::enumeration_snapshot();
        let mut store = AccessStore::new(&snapshot);
        store.set_users(users);
        assert_eq!(
            store.users().unwrap()[0].data.interface_language,
            expected.map(|guid| Guid::from_str(guid).unwrap())
        );
    }
}

#[test]
fn shared_rows_retain_all_six_native_legacy_languages() {
    for layout in [31, 37] {
        for (name, expected) in [
            ("LangRussian", "855375b9-327b-53fa-b11c-6454c2c7a1a6"),
            ("LangEnglish", "220d033d-4a5f-45ed-926b-af14b6cf30bc"),
            ("LangDefault", "00000000-0000-0000-0000-000000000000"),
        ] {
            let bytes = std::fs::read(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../tests/fixtures/user-interface-language/legacy/inputs")
                    .join(format!("{name}.legacy-{layout}.data")),
            )
            .unwrap();
            let rows = vec![vec![
                Cell::Text(name.into()),
                Cell::Text("own user".into()),
                Cell::Text(String::new()),
                Cell::Number("1".into()),
                Cell::Number("1".into()),
                Cell::Number("0".into()),
                Cell::Bytes(bytes),
            ]];
            let users = users_from_rows(&rows).unwrap();
            assert_eq!(
                users[0].data.interface_language,
                Some(Guid::from_str(expected).unwrap())
            );
        }
    }
}
