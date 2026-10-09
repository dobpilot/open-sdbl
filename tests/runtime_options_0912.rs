mod support;
use open_sdbl::metadata::{
    CommonAttributeConditionalSeparation, CommonAttributeSeparation, ConfigDescriptor, DbNameEntry,
    DbNames, FunctionalOptionLocation, SchemaStorage, parse_config_descriptors, resolve_metadata,
};
use support::stored_deflate;
fn descriptors(label: &str) -> Vec<ConfigDescriptor> {
    let base = format!(
        "{}/tests/fixtures/runtime-options-0912/{label}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(format!("{base}/resources.txt"))
        .unwrap()
        .lines()
        .flat_map(|id| {
            parse_config_descriptors(id, &std::fs::read(format!("{base}/{id}.deflate")).unwrap())
                .unwrap()
        })
        .collect()
}
#[test]
fn native_sources_flags_and_complete_parameter_membership_survive_resolution() {
    for (label, count) in [("native", 0), ("catalog-native", 1), ("register-native", 2)] {
        let ds = descriptors(label);
        let roots = ds
            .iter()
            .filter_map(|d| d.configuration_functional_option_parameters.as_ref())
            .collect::<Vec<_>>();
        assert_eq!(roots.len(), 1, "{label}");
        assert_eq!(roots[0].len(), count);
        for id in roots[0] {
            assert!(
                ds.iter()
                    .find(|d| &d.object_guid == id)
                    .unwrap()
                    .functional_option_parameter_use
                    .is_some()
            );
        }
        let db_names = DbNames::from_entries(if label == "native" {
            vec![DbNameEntry {
                guid: "9893e2d6-f3f8-4d73-bb06-19bf26d216ab".parse().unwrap(),
                alias: "Const".into(),
                number: 1,
            }]
        } else {
            vec![]
        });
        let snap = resolve_metadata(
            db_names,
            ds,
            SchemaStorage {
                tables: vec![],
                anomalies: vec![],
            },
            vec![],
        )
        .snapshot;
        let options = snap
            .descriptors()
            .iter()
            .filter(|d| d.functional_option_location_guid.is_some())
            .collect::<Vec<_>>();
        assert_eq!(options.len(), 2, "{label}");
        for d in options {
            assert!(d.functional_option_privileged_get_mode.is_some());
            match (&d.functional_option_location, label) {
                (Some(FunctionalOptionLocation::Constant(id)), "native") => {
                    assert_eq!(Some(id), d.functional_option_location_guid.as_ref())
                }
                (
                    Some(FunctionalOptionLocation::CatalogAttribute { owner, attribute }),
                    "catalog-native",
                ) => {
                    assert_eq!(owner.as_str(), "a7177622-2cb3-5bfd-a0d7-1ae7a4a9f4d5");
                    assert_eq!(Some(attribute), d.functional_option_location_guid.as_ref());
                }
                (
                    Some(FunctionalOptionLocation::InformationRegisterResource { owner, resource }),
                    "register-native",
                ) => {
                    assert_eq!(owner.as_str(), "c34d5785-6f04-45c7-9555-088544ddc489");
                    assert_eq!(Some(resource), d.functional_option_location_guid.as_ref());
                }
                other => panic!("unexpected native source: {other:?}"),
            }
        }
    }
}
#[test]
fn native_disabled_and_separate_are_independent_of_present_settings() {
    for (label, expected) in [
        ("common-disabled", CommonAttributeSeparation::DontUse),
        ("common-separate", CommonAttributeSeparation::Separate),
    ] {
        let ds = descriptors(label);
        let d = ds
            .iter()
            .find(|d| d.object_guid.as_str() == "03f9d2d0-eae4-4098-a70e-4eb569b11fd9")
            .unwrap();
        assert_eq!(d.common_attribute_separation, Some(expected));
        assert!(d.separation.is_some());
        let usage = d.common_attribute_usage.as_ref().unwrap();
        assert_eq!(usage.content.len(), 1);
        assert_eq!(
            usage.content[0].conditional_separation,
            Some(CommonAttributeConditionalSeparation::Undefined)
        );
    }
}
#[test]
fn unsupported_flags_links_and_parameter_count_never_become_defaults_or_partial_success() {
    let base = format!(
        "{}/tests/fixtures/runtime-options-0912",
        env!("CARGO_MANIFEST_DIR")
    );
    let id = "a11dfe2b-ef68-4701-9977-08fb0e680ee1";
    let text = std::fs::read_to_string(format!("{base}/native/{id}.config")).unwrap();
    let bad = text.replace("{0,0},0}", "{0,0},9}");
    assert_ne!(text, bad);
    let ds = parse_config_descriptors(id, &stored_deflate(bad.as_bytes())).unwrap();
    assert!(
        ds.iter()
            .find(|d| d.object_guid.as_str() == id)
            .unwrap()
            .functional_option_privileged_get_mode
            .is_none()
    );
    let id = "a12e740b-47ae-4eca-9ea0-2a7e49a23b99";
    let text = std::fs::read_to_string(format!("{base}/catalog-native/{id}.config")).unwrap();
    let bad = text.replace("{0,1,", "{0,2,");
    assert_ne!(text, bad);
    let ds = parse_config_descriptors(id, &stored_deflate(bad.as_bytes())).unwrap();
    assert!(
        ds.iter()
            .find(|d| d.object_guid.as_str() == id)
            .unwrap()
            .functional_option_parameter_use
            .is_none()
    );
}
fn resolved(label: &str) -> open_sdbl::metadata::MetadataSnapshot {
    resolve_metadata(
        DbNames::from_entries(vec![]),
        descriptors(label),
        SchemaStorage {
            tables: vec![],
            anomalies: vec![],
        },
        vec![],
    )
    .snapshot
}
#[test]
fn complete_runtime_content_matches_native_class_applicability_order_and_condition() {
    for (label, count) in [
        ("condition", 3),
        ("order", 5),
        ("order-reversed", 5),
        ("separate", 14),
        ("disabled", 15),
        ("inapplicable", 15),
        ("scheduled-separate", 15),
        ("scheduled-disabled", 15),
    ] {
        let snap = resolved(label);
        let common = snap
            .descriptors()
            .iter()
            .find(|d| d.object_guid.as_str() == "03f9d2d0-eae4-4098-a70e-4eb569b11fd9")
            .unwrap();
        let content = common.common_attribute_content.as_ref().expect(label);
        assert_eq!(content.len(), count, "{label}");
        let oracle = std::fs::read_to_string(format!(
            "{}/tests/fixtures/runtime-options-0912/{label}/content-oracle.tsv",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let actual = content
            .iter()
            .map(|entry| {
                format!(
                    "{}\t{:?}\t{}",
                    entry.object,
                    entry.usage,
                    match &entry.conditional_separation {
                        Some(CommonAttributeConditionalSeparation::Undefined) => "undefined".into(),
                        Some(CommonAttributeConditionalSeparation::Constant(id)) => id.to_string(),
                        None => "unknown".into(),
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        assert_eq!(actual, oracle, "{label}");
    }
}
#[test]
fn missing_inventory_member_or_unknown_separation_never_produces_partial_content() {
    let ds = descriptors("separate");
    let root = ds
        .iter()
        .find(|d| d.configuration_common_attribute_candidates.is_some())
        .unwrap();
    let missing = root
        .configuration_common_attribute_candidates
        .as_ref()
        .unwrap()[0]
        .object
        .clone();
    let absent = ds
        .iter()
        .filter(|d| d.object_guid != missing)
        .cloned()
        .collect();
    let snap = resolve_metadata(
        DbNames::from_entries(vec![]),
        absent,
        SchemaStorage {
            tables: vec![],
            anomalies: vec![],
        },
        vec![],
    )
    .snapshot;
    assert!(
        snap.descriptors()
            .iter()
            .find(|d| d.common_attribute_usage.is_some())
            .unwrap()
            .common_attribute_content
            .is_none()
    );
    for unknown_root in [true, false] {
        let mut ds = ds.clone();
        if unknown_root {
            for d in &mut ds {
                d.configuration_common_attribute_candidates = None;
            }
        } else {
            for d in &mut ds {
                d.common_attribute_separation = None;
            }
        }
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
        assert!(
            snap.descriptors()
                .iter()
                .find(|d| d.common_attribute_usage.is_some())
                .unwrap()
                .common_attribute_content
                .is_none()
        );
    }
}
#[test]
fn unknown_parameter_use_invalidates_complete_root_membership() {
    let mut ds = descriptors("register-native");
    ds.iter_mut()
        .find(|d| d.functional_option_parameter_use.is_some())
        .unwrap()
        .functional_option_parameter_use = None;
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
    assert!(
        snap.descriptors()
            .iter()
            .all(|d| d.configuration_functional_option_parameters.is_none())
    );
}
#[test]
fn native_catalog_use_register_profiles_explicit_auto_and_multiple_links_are_preserved() {
    use open_sdbl::metadata::{
        CatalogAttributeUse, CommonAttributeContentUse, InformationRegisterPeriodicity,
        InformationRegisterWriteMode,
    };
    let snap = resolved("settings");
    for (name, usage) in [
        ("ПробаForItem", CatalogAttributeUse::ForItem),
        ("ПробаForFolder", CatalogAttributeUse::ForFolder),
        (
            "ПробаForFolderAndItem",
            CatalogAttributeUse::ForFolderAndItem,
        ),
    ] {
        assert_eq!(
            snap.descriptors()
                .iter()
                .find(|d| d.name == name)
                .unwrap()
                .catalog_attribute_use,
            Some(usage)
        );
    }
    for (name, mode, period) in [
        (
            "ОпцииПробы",
            InformationRegisterWriteMode::Independent,
            InformationRegisterPeriodicity::Nonperiodical,
        ),
        (
            "ПробаДень",
            InformationRegisterWriteMode::Independent,
            InformationRegisterPeriodicity::Day,
        ),
        (
            "ПробаРегистратор",
            InformationRegisterWriteMode::RecorderSubordinate,
            InformationRegisterPeriodicity::Nonperiodical,
        ),
    ] {
        let d = snap.descriptors().iter().find(|d| d.name == name).unwrap();
        assert_eq!(d.information_register_write_mode, Some(mode));
        assert_eq!(d.information_register_periodicity, Some(period));
    }
    let parameter = snap
        .descriptors()
        .iter()
        .find(|d| d.name == "ПробаДвухСвязей")
        .unwrap();
    assert_eq!(
        parameter
            .functional_option_parameter_use
            .as_ref()
            .unwrap()
            .len(),
        2
    );
    let common = snap
        .descriptors()
        .iter()
        .find(|d| d.common_attribute_usage.is_some())
        .unwrap();
    assert_eq!(
        common.common_attribute_usage.as_ref().unwrap().content[0].usage,
        CommonAttributeContentUse::Auto
    );
    assert_eq!(common.common_attribute_content.as_ref().unwrap().len(), 16);
}
#[test]
fn malformed_root_inventory_does_not_claim_completeness() {
    let base = format!(
        "{}/tests/fixtures/runtime-options-0912/separate",
        env!("CARGO_MANIFEST_DIR")
    );
    let ds = descriptors("separate");
    let root_id = ds
        .iter()
        .find(|d| d.configuration_common_attribute_candidates.is_some())
        .unwrap()
        .resource_guid
        .clone();
    let text = std::fs::read_to_string(format!("{base}/{root_id}.config")).unwrap();
    for (old, new) in [
        (
            "0195e80c-b157-11d4-9435-004095e12fc7",
            "ffffffff-ffff-ffff-ffff-ffffffffffff",
        ),
        (
            "9893e2d6-f3f8-4d73-bb06-19bf26d216ab",
            "00000000-0000-0000-0000-000000000000",
        ),
    ] {
        let bad = text.replace(old, new);
        assert_ne!(text, bad);
        let decoded =
            parse_config_descriptors(root_id.as_str(), &stored_deflate(bad.as_bytes())).unwrap();
        assert!(
            decoded
                .iter()
                .all(|d| d.configuration_common_attribute_candidates.is_none())
        );
    }
}
#[test]
fn unknown_nonzero_condition_remains_unknown_without_losing_verified_membership() {
    let mut ds = descriptors("condition");
    let common = ds
        .iter_mut()
        .find(|d| d.common_attribute_usage.is_some())
        .unwrap();
    common.common_attribute_usage.as_mut().unwrap().content[0].conditional_separation_guid =
        "ffffffff-ffff-ffff-ffff-ffffffffffff".parse().unwrap();
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
    let common = snap
        .descriptors()
        .iter()
        .find(|d| d.common_attribute_usage.is_some())
        .unwrap();
    let raw = &common.common_attribute_usage.as_ref().unwrap().content[0];
    assert_eq!(raw.conditional_separation, None);
    let full = common.common_attribute_content.as_ref().unwrap();
    assert_eq!(full.len(), 3);
    assert_eq!(
        full.iter()
            .find(|d| d.object == raw.object)
            .unwrap()
            .conditional_separation,
        None
    );
}
#[test]
fn native_compiler_discards_inapplicable_explicit_constant_from_disabled_content() {
    let snap = resolved("inapplicable");
    let common = snap
        .descriptors()
        .iter()
        .find(|d| d.common_attribute_usage.is_some())
        .unwrap();
    let constant: open_sdbl::metadata::Guid =
        "9893e2d6-f3f8-4d73-bb06-19bf26d216ab".parse().unwrap();
    assert!(
        common
            .common_attribute_usage
            .as_ref()
            .unwrap()
            .content
            .iter()
            .all(|d| d.object != constant)
    );
    let content = common.common_attribute_content.as_ref().unwrap();
    assert_eq!(content.len(), 15);
    assert!(content.iter().all(|d| d.object != constant));
}
#[test]
fn complete_information_register_membership_is_not_reconstructed_from_surviving_fields() {
    let snap = resolved("register-native");
    let owner = snap
        .descriptors()
        .iter()
        .find(|d| d.object_guid.as_str() == "c34d5785-6f04-45c7-9555-088544ddc489")
        .unwrap();
    let dims = owner.information_register_dimensions.as_ref().unwrap();
    let resources = owner.information_register_resources.as_ref().unwrap();
    assert_eq!(
        dims.iter().map(|id| id.as_str()).collect::<Vec<_>>(),
        [
            "9852d238-b149-4520-9745-75b0234529c8",
            "b38a5ccc-45e8-4f1f-8d94-15b3bbd34e25"
        ]
    );
    assert_eq!(
        resources.iter().map(|id| id.as_str()).collect::<Vec<_>>(),
        [
            "9d1fa913-1a38-4ee4-a9d5-763eaced5a0d",
            "709ba1ee-c392-42ad-afa0-72deb98d83f0"
        ]
    );
    for missing in [dims[0].clone(), resources[0].clone()] {
        let ds = descriptors("register-native")
            .into_iter()
            .filter(|d| d.object_guid != missing)
            .collect();
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
        let owner = snap
            .descriptors()
            .iter()
            .find(|d| d.object_guid.as_str() == "c34d5785-6f04-45c7-9555-088544ddc489")
            .unwrap();
        if missing == dims[0] {
            assert!(owner.information_register_dimensions.is_none());
            assert!(owner.information_register_resources.is_some());
        } else {
            assert!(owner.information_register_resources.is_none());
            assert!(owner.information_register_dimensions.is_some());
        }
    }
    let id = "c34d5785-6f04-45c7-9555-088544ddc489";
    let text = std::fs::read_to_string(format!(
        "{}/tests/fixtures/runtime-options-0912/register-native/{id}.config",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    for (old, new) in [
        (
            "{13134203-f60b-11d5-a3c7-0050bae0a776,2,",
            "{13134203-f60b-11d5-a3c7-0050bae0a776,3,",
        ),
        (
            "b38a5ccc-45e8-4f1f-8d94-15b3bbd34e25",
            "9852d238-b149-4520-9745-75b0234529c8",
        ),
    ] {
        let bad = text.replace(old, new);
        assert_ne!(text, bad);
        let ds = parse_config_descriptors(id, &stored_deflate(bad.as_bytes())).unwrap();
        assert!(
            ds.iter()
                .find(|d| d.object_guid.as_str() == id)
                .unwrap()
                .information_register_dimensions
                .is_none()
        );
    }
}
