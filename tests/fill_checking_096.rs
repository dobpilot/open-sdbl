use open_sdbl::metadata::{
    DbNames, FillChecking, SchemaStorage, parse_config_descriptors, resolve_metadata,
};
use std::collections::HashMap;

#[test]
fn native_fill_checking_matches_every_verified_owner_and_field_through_the_snapshot() {
    let oracle = include_str!("fixtures/fill-checking-096/native.tsv");
    let mut standard_lists = 0;
    let mut standard_modes = [0; 2];
    let mut field_modes = [0; 2];
    let mut common = 0;
    let mode = |value| match value {
        "DontCheck" => FillChecking::DontCheck,
        "ShowError" => FillChecking::ShowError,
        _ => panic!(),
    };
    for (db, pack) in [
        (
            "demo",
            include_bytes!("fixtures/demo/config.pack").as_slice(),
        ),
        ("buh", include_bytes!("fixtures/buh/config.pack").as_slice()),
        ("unf", include_bytes!("fixtures/unf/config.pack").as_slice()),
    ] {
        let mut expected = HashMap::<&str, Vec<Vec<&str>>>::new();
        for row in oracle.lines() {
            let fields: Vec<_> = row.split('\t').collect();
            if fields[0] == db {
                expected.entry(fields[1]).or_default().push(fields);
            }
        }
        let mut offset = 0;
        while offset < pack.len() {
            let end = offset + pack[offset..].iter().position(|b| *b == b'\n').unwrap();
            let (resource, size) = std::str::from_utf8(&pack[offset..end])
                .unwrap()
                .split_once('\t')
                .unwrap();
            let size: usize = size.parse().unwrap();
            offset = end + 1;
            if let Some(rows) = expected.get(resource) {
                let snapshot = resolve_metadata(
                    DbNames::from_entries(Vec::new()),
                    parse_config_descriptors(resource, &pack[offset..offset + size]).unwrap(),
                    SchemaStorage {
                        tables: Vec::new(),
                        anomalies: Vec::new(),
                    },
                    Vec::new(),
                )
                .snapshot;
                for row in rows {
                    let descriptor = snapshot
                        .descriptors()
                        .iter()
                        .find(|d| d.object_guid.as_str() == row[2])
                        .unwrap_or_else(|| panic!("{db}:{resource}:{}", row[2]));
                    if row[3] == "Standard" {
                        let expected: Vec<_> = row[4]
                            .split(';')
                            .map(|field| {
                                let (name, value) = field.split_once(':').unwrap();
                                (name, mode(value))
                            })
                            .collect();
                        let actual: Vec<_> = descriptor
                            .catalog_standard_fill_checking
                            .as_ref()
                            .unwrap_or_else(|| panic!("{db}:{resource}"))
                            .iter()
                            .map(|field| (field.name.as_str(), field.fill_checking))
                            .collect();
                        assert_eq!(actual, expected, "{db}:{resource}");
                        for (_, value) in actual {
                            standard_modes[usize::from(value == FillChecking::ShowError)] += 1;
                        }
                        standard_lists += 1;
                    } else {
                        let value = mode(row[4]);
                        assert_eq!(
                            descriptor.fill_checking,
                            Some(value),
                            "{db}:{resource}:{}",
                            row[2]
                        );
                        field_modes[usize::from(value == FillChecking::ShowError)] += 1;
                        common += usize::from(row[3] == "Common");
                    }
                }
            }
            offset += size;
        }
    }
    assert_eq!(standard_lists, 728);
    assert_eq!(field_modes, [26180, 5154]);
    assert_eq!(common, 27);
    assert!(standard_modes[0] > 0 && standard_modes[1] > 0);
}

#[test]
fn native_country_settings_do_not_guess_code_or_description_defaults() {
    let ds = parse_config_descriptors(
        "5b2a2ccb-a948-4628-b145-1ce5696fdfe0",
        include_bytes!("fixtures/fill-checking-096/СтраныМира.deflate"),
    )
    .unwrap();
    let owner = ds
        .iter()
        .find(|d| d.object_guid.as_str() == "5b2a2ccb-a948-4628-b145-1ce5696fdfe0")
        .unwrap();
    let settings = owner.catalog_standard_fill_checking.as_ref().unwrap();
    assert_eq!(settings.len(), 9);
    assert_eq!(
        settings
            .iter()
            .find(|setting| setting.name == "Code")
            .unwrap()
            .fill_checking,
        FillChecking::ShowError
    );
    assert_eq!(
        settings
            .iter()
            .find(|setting| setting.name == "Description")
            .unwrap()
            .fill_checking,
        FillChecking::ShowError
    );
    assert!(
        ds.iter()
            .filter(|d| d.object_guid != owner.object_guid)
            .all(|d| d.catalog_standard_fill_checking.is_none())
    );
}

#[test]
fn malformed_unknown_and_sparse_standard_settings_are_not_partial_or_empty() {
    for bytes in [
        include_bytes!("fixtures/fill-checking-096/standard-unknown-mode.deflate").as_slice(),
        include_bytes!("fixtures/fill-checking-096/standard-unknown-name.deflate").as_slice(),
        include_bytes!("fixtures/fill-checking-096/standard-bad-count.deflate").as_slice(),
        include_bytes!("fixtures/fill-checking-096/standard-duplicate-name.deflate").as_slice(),
        include_bytes!("fixtures/fill-checking-096/standard-sparse.deflate").as_slice(),
    ] {
        let ds = parse_config_descriptors("5b2a2ccb-a948-4628-b145-1ce5696fdfe0", bytes).unwrap();
        assert!(
            ds.iter()
                .find(|d| d.object_guid.as_str() == "5b2a2ccb-a948-4628-b145-1ce5696fdfe0")
                .unwrap()
                .catalog_standard_fill_checking
                .is_none()
        );
    }
}

#[test]
fn an_unknown_attribute_mode_remains_unknown_without_erasing_usage_and_type() {
    let ds = parse_config_descriptors(
        "03f9d2d0-eae4-4098-a70e-4eb569b11fd9",
        include_bytes!("fixtures/fill-checking-096/field-unknown-mode.deflate"),
    )
    .unwrap();
    let field = ds
        .iter()
        .find(|d| d.object_guid.as_str() == "03f9d2d0-eae4-4098-a70e-4eb569b11fd9")
        .unwrap();
    assert!(field.fill_checking.is_none());
    assert!(field.declared_types.is_some());
    assert!(field.common_attribute_usage.is_some());
}
