mod support;
use open_sdbl::metadata::{
    AccumulationRegisterKind, CatalogCodeType, ConfigDescriptor, DbNames, FillChecking,
    SchemaStorage, parse_config_descriptors, resolve_metadata,
};
use support::stored_deflate;
const BP: &str = "6db4bbe0-ce4c-4389-ad25-9a10c1857192";
fn parse(id: &str, text: &str) -> Vec<ConfigDescriptor> {
    parse_config_descriptors(id, &stored_deflate(text.as_bytes())).unwrap()
}
fn owner<'a>(ds: &'a [ConfigDescriptor], id: &str) -> &'a ConfigDescriptor {
    ds.iter().find(|d| d.object_guid.as_str() == id).unwrap()
}
fn names(ds: &[ConfigDescriptor], ids: &Option<Vec<open_sdbl::metadata::Guid>>) -> Vec<String> {
    ids.as_ref()
        .unwrap()
        .iter()
        .map(|id| {
            ds.iter()
                .find(|d| &d.object_guid == id)
                .unwrap()
                .name
                .clone()
        })
        .collect()
}
#[test]
fn native_business_process_members_and_types_are_complete_and_separate() {
    let ds = parse(
        BP,
        include_str!("fixtures/runtime-object-099/tabular-section.native.config"),
    );
    let d = owner(&ds, BP);
    assert_eq!(
        names(&ds, &d.business_process_attributes),
        ["ТекстПробы", "ФлагПробы"]
    );
    assert_eq!(
        names(&ds, &d.business_process_tabular_sections),
        ["СтрокиПробы"]
    );
    let section = ds.iter().find(|d| d.name == "СтрокиПробы").unwrap();
    assert!(section.tabular_section_attributes.is_some(), "{section:?}");
    assert_eq!(
        names(&ds, &section.tabular_section_attributes),
        ["ТекстСтроки", "ФлагСтроки"]
    );
    for id in d
        .business_process_attributes
        .as_ref()
        .unwrap()
        .iter()
        .chain(section.tabular_section_attributes.as_ref().unwrap())
    {
        let a = ds.iter().find(|d| &d.object_guid == id).unwrap();
        assert!(a.declared_types.is_some());
        assert!(a.fill_checking.is_some());
        assert_eq!(a.fill_from_filling_value, Some(false));
        assert!(a.filling_value.is_some());
        assert!(!a.synonyms.is_empty());
        assert!(a.business_process_attributes.is_none());
    }
    assert_eq!(
        d.business_process_object_type.as_ref().unwrap().as_str(),
        "3d8f31b8-65d2-4492-933e-1fd4c8a041f1"
    );
    assert_ne!(d.business_process_object_type, d.object_reference_type);
    assert!(d.catalog_attributes.is_none());
    assert!(d.document_attributes.is_none());
    assert!(d.accumulation_register_kind.is_none());
}
#[test]
fn native_number_variants_and_standard_keys_match_independent_xml_controls() {
    for (text, kind, len, var) in [
        (
            include_str!("fixtures/runtime-object-099/own-fields.native.config"),
            CatalogCodeType::String,
            11,
            true,
        ),
        (
            include_str!("fixtures/runtime-object-099/number.config"),
            CatalogCodeType::Number,
            8,
            false,
        ),
        (
            include_str!("fixtures/runtime-object-099/fixed.config"),
            CatalogCodeType::String,
            9,
            false,
        ),
    ] {
        let ds = parse(BP, text);
        let d = owner(&ds, BP);
        let n = d.business_process_number.unwrap();
        assert_eq!(
            (n.number_type, n.length, n.variable_length),
            (kind, len, var)
        );
        let policy = d.business_process_standard_fill_checking.as_ref().unwrap();
        assert_eq!(
            policy.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
            [
                "Started",
                "HeadTask",
                "Completed",
                "Ref",
                "DeletionMark",
                "Date",
                "Number"
            ]
        );
        assert!(
            policy
                .iter()
                .all(|r| r.fill_checking == FillChecking::DontCheck)
        );
    }
    let ds = parse(
        BP,
        include_str!("fixtures/runtime-object-099/standard.config"),
    );
    assert_eq!(
        owner(&ds, BP)
            .business_process_standard_fill_checking
            .as_ref()
            .unwrap()
            .len(),
        7
    );
    for (text, flag, check) in [
        (
            include_str!("fixtures/runtime-object-099/own-fields.native.config"),
            false,
            FillChecking::DontCheck,
        ),
        (
            include_str!("fixtures/runtime-object-099/fill-enabled.native.config"),
            true,
            FillChecking::DontCheck,
        ),
        (
            include_str!("fixtures/runtime-object-099/check-filling-required.native.config"),
            false,
            FillChecking::ShowError,
        ),
    ] {
        let ds = parse(BP, text);
        let a = ds.iter().find(|d| d.name == "ТекстПробы").unwrap();
        assert_eq!(a.fill_from_filling_value, Some(flag));
        assert_eq!(a.fill_checking, Some(check));
    }
    let ds = parse(
        BP,
        include_str!(
            "fixtures/business-task-reference/6db4bbe0-ce4c-4389-ad25-9a10c1857192.native.config"
        ),
    );
    let d = owner(&ds, BP);
    assert_eq!(d.business_process_attributes, Some(vec![]));
    assert_eq!(d.business_process_tabular_sections, Some(vec![]));
}
#[test]
fn native_register_kinds_order_and_empty_members_match_xml_oracle() {
    for row in include_str!("fixtures/runtime-object-099/registers.tsv").lines() {
        let c = row.split('\t').collect::<Vec<_>>();
        let path = format!(
            "{}/tests/fixtures/runtime-object-099/{}.deflate",
            env!("CARGO_MANIFEST_DIR"),
            c[1]
        );
        let ds = parse_config_descriptors(c[0], &std::fs::read(path).unwrap()).unwrap();
        let d = owner(&ds, c[0]);
        assert_eq!(
            d.accumulation_register_kind,
            Some(if c[2] == "Balance" {
                AccumulationRegisterKind::Balance
            } else {
                AccumulationRegisterKind::Turnovers
            })
        );
        for (actual, expected) in [
            (&d.accumulation_register_dimensions, c[3]),
            (&d.accumulation_register_resources, c[4]),
        ] {
            assert_eq!(
                actual
                    .as_ref()
                    .unwrap()
                    .iter()
                    .map(|g| g.as_str())
                    .collect::<Vec<_>>(),
                expected
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
            );
            for id in actual.as_ref().unwrap() {
                let a = ds.iter().find(|a| &a.object_guid == id).unwrap();
                assert!(a.declared_types.is_some());
                assert!(!a.synonyms.is_empty());
            }
        }
        assert!(d.business_process_attributes.is_none());
        assert!(d.document_attributes.is_none());
        let old = d.clone();
        let snapshot = resolve_metadata(
            DbNames::from_entries(vec![]),
            ds,
            SchemaStorage {
                tables: vec![],
                anomalies: vec![],
            },
            vec![],
        )
        .snapshot;
        let new = owner(snapshot.descriptors(), c[0]);
        assert_eq!(
            old.accumulation_register_kind,
            new.accumulation_register_kind
        );
        assert_eq!(
            old.accumulation_register_dimensions,
            new.accumulation_register_dimensions
        );
        assert_eq!(
            old.accumulation_register_resources,
            new.accumulation_register_resources
        );
    }
}
#[test]
fn corrupted_members_and_settings_are_unknown_without_partial_success() {
    let text = include_str!("fixtures/runtime-object-099/tabular-section.native.config");
    for (old, new) in [
        (
            "87c988de-ecbf-413b-87b0-b9516df05e28,2",
            "87c988de-ecbf-413b-87b0-b9516df05e28,3",
        ),
        (
            "3ee1e946-c861-4385-87e1-4bd3eb53803d",
            "10c27c5d-bc59-4440-9a01-11cb7a0e1dcd",
        ),
        ("\"Pattern\",", "\"UnknownPattern\","),
    ] {
        assert!(text.contains(old));
        let ds = parse(BP, &text.replacen(old, new, 1));
        assert!(owner(&ds, BP).business_process_attributes.is_none());
    }
    let row = include_str!("fixtures/runtime-object-099/registers.tsv")
        .lines()
        .find(|r| r.contains("ОстаткиПорядок"))
        .unwrap()
        .split('\t')
        .collect::<Vec<_>>();
    let text = include_str!("fixtures/runtime-object-099/ОстаткиПорядок.config");
    let bad = text.replacen(
        "b64d9a43-1642-11d6-a3c7-0050bae0a776,2",
        "b64d9a43-1642-11d6-a3c7-0050bae0a776,3",
        1,
    );
    assert_ne!(bad, text);
    let ds = parse(row[0], &bad);
    let d = owner(&ds, row[0]);
    assert!(d.accumulation_register_dimensions.is_none());
    assert_eq!(d.accumulation_register_resources.as_ref().unwrap().len(), 2);
}
#[test]
fn business_process_acquisition_survives_snapshot_and_tasks_remain_unaffected() {
    let ds = parse(
        BP,
        include_str!("fixtures/runtime-object-099/tabular-section.native.config"),
    );
    let old = owner(&ds, BP).clone();
    let snap = resolve_metadata(
        DbNames::from_entries(vec![]),
        ds,
        SchemaStorage {
            tables: vec![],
            anomalies: vec![],
        },
        vec![],
    )
    .snapshot;
    let new = owner(snap.descriptors(), BP);
    assert_eq!(old.business_process_number, new.business_process_number);
    assert_eq!(
        old.business_process_attributes,
        new.business_process_attributes
    );
    assert_eq!(
        old.business_process_tabular_sections,
        new.business_process_tabular_sections
    );
    assert_eq!(
        old.business_process_standard_fill_checking,
        new.business_process_standard_fill_checking
    );
    let id = "2adee501-a0c9-4849-9fd0-a3d10679d4f7";
    let ds = parse(
        id,
        include_str!(
            "fixtures/business-task-reference/2adee501-a0c9-4849-9fd0-a3d10679d4f7.native.config"
        ),
    );
    let d = owner(&ds, id);
    assert!(d.business_process_number.is_none());
    assert!(d.accumulation_register_kind.is_none());
    assert!(d.business_process_standard_fill_checking.is_none());
}

#[test]
fn native_standard_required_setting_preserves_other_policies() {
    let ds = parse(
        BP,
        include_str!("fixtures/runtime-object-099/standard-required.config"),
    );
    let policies = owner(&ds, BP)
        .business_process_standard_fill_checking
        .as_ref()
        .unwrap();
    for policy in policies {
        assert_eq!(
            policy.fill_checking,
            if policy.name == "Number" {
                FillChecking::ShowError
            } else {
                FillChecking::DontCheck
            }
        );
    }
}

#[test]
fn three_native_sections_preserve_order_separate_fields_and_verified_empty_membership() {
    let ds = parse(
        BP,
        include_str!("fixtures/runtime-object-099/membership.config"),
    );
    let d = owner(&ds, BP);
    assert_eq!(
        names(&ds, &d.business_process_tabular_sections),
        ["СтрокиПробы", "ДругаяЧасть", "ПустаяЧасть"]
    );
    let mut seen = std::collections::HashSet::new();
    for row in include_str!("fixtures/runtime-object-099/sections.tsv").lines() {
        let cols = row.split('\t').collect::<Vec<_>>();
        let section = owner(&ds, cols[0]);
        assert_eq!(section.name, cols[1]);
        let expected = cols[2]
            .split(',')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();
        let actual = section.tabular_section_attributes.as_ref().unwrap();
        assert_eq!(
            actual.iter().map(|g| g.as_str()).collect::<Vec<_>>(),
            expected
        );
        for id in actual {
            assert!(seen.insert(id.clone()));
            assert!(!d.business_process_attributes.as_ref().unwrap().contains(id));
        }
    }
    assert_eq!(seen.len(), 4);
}
