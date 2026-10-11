//! Назначение из нативного configuration-root, отдельно от SQL-флагов.

use open_sdbl::metadata::{ConfigurationExtensionPurpose, parse_extension_catalog};

fn resource(kind: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/extension/purpose-0915/{kind}.serialized",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

#[test]
fn all_native_purposes_match_installed_extension_info_and_xml() {
    for (kind, purpose, xml_name) in [
        ("patch", ConfigurationExtensionPurpose::Patch, "Patch"),
        (
            "customization",
            ConfigurationExtensionPurpose::Customization,
            "Customization",
        ),
        ("add-on", ConfigurationExtensionPurpose::AddOn, "AddOn"),
    ] {
        let root = format!(
            "{}/tests/fixtures/extension/purpose-0915",
            env!("CARGO_MANIFEST_DIR")
        );
        let info = std::fs::read_to_string(format!("{root}/{kind}-info.txt")).unwrap();
        assert!(info.lines().any(|line| {
            line.split_once(':')
                .is_some_and(|(key, value)| key.trim() == "purpose" && value.trim() == kind)
        }));
        let xml = std::fs::read_to_string(format!("{root}/{kind}.xml")).unwrap();
        assert!(xml.contains(&format!(
            "<ConfigurationExtensionPurpose>{xml_name}</ConfigurationExtensionPurpose>"
        )));
        assert_eq!(
            parse_extension_catalog(&resource(kind)).unwrap().purpose,
            Some(purpose)
        );
    }
}

#[test]
fn unknown_purpose_and_unmeasured_header_keep_the_catalog_without_a_default() {
    let text = String::from_utf8(resource("patch")).unwrap();
    for (before, after) in [
        ("},2,\"\",80327,0,0,", "},2,\"\",80327,9,0,"),
        ("},2,\"\",80327,0,0,", "},2,\"\",80328,0,0,"),
        ("{68,", "{69,"),
    ] {
        assert!(text.contains(before));
        let changed = text.replacen(before, after, 1);
        let catalog = parse_extension_catalog(changed.as_bytes()).unwrap();
        assert_eq!(catalog.purpose, None);
        assert!(!catalog.classes.is_empty());
    }
}

#[test]
fn damaged_and_truncated_native_roots_never_invent_purpose() {
    let bytes = resource("patch");
    for end in 0..bytes.len() - 1 {
        assert!(
            parse_extension_catalog(&bytes[..end])
                .ok()
                .and_then(|c| c.purpose)
                .is_none(),
            "{end}"
        );
    }
}

#[test]
fn name_changes_do_not_change_purpose() {
    let text = String::from_utf8(resource("patch")).unwrap();
    let text = text.replace("Probe0915Patch", "DefinitelyNotEF");
    assert_eq!(
        parse_extension_catalog(text.as_bytes()).unwrap().purpose,
        Some(ConfigurationExtensionPurpose::Patch)
    );
}

#[test]
fn older_native_compatibility_modes_confirm_the_same_three_codes() {
    for compatibility in ["80324", "80317"] {
        for (kind, purpose, xml_name) in [
            ("patch", ConfigurationExtensionPurpose::Patch, "Patch"),
            (
                "customization",
                ConfigurationExtensionPurpose::Customization,
                "Customization",
            ),
            ("add-on", ConfigurationExtensionPurpose::AddOn, "AddOn"),
        ] {
            let label = format!("{kind}-{compatibility}");
            let root = format!(
                "{}/tests/fixtures/extension/purpose-0915",
                env!("CARGO_MANIFEST_DIR")
            );
            let xml = std::fs::read_to_string(format!("{root}/{label}.xml")).unwrap();
            assert!(xml.contains(&format!(
                "<ConfigurationExtensionPurpose>{xml_name}</ConfigurationExtensionPurpose>"
            )));
            assert!(xml.contains(&format!("<ConfigurationExtensionCompatibilityMode>Version8_3_{}</ConfigurationExtensionCompatibilityMode>",&compatibility[3..])));
            let info = std::fs::read_to_string(format!("{root}/{label}-info.txt")).unwrap();
            assert!(info.lines().any(|line| {
                line.split_once(':')
                    .is_some_and(|(k, v)| k.trim() == "purpose" && v.trim() == kind)
            }));
            assert_eq!(
                parse_extension_catalog(&resource(&label)).unwrap().purpose,
                Some(purpose)
            );
        }
    }
}
