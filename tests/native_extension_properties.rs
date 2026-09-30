//! Измеренные свойства расширения: native 1C 8.3.27.2342 и сырые записи SQL.

use open_sdbl::metadata::parse_extension_info;
use std::path::PathBuf;

fn native(label: &str) -> (Vec<u8>, String) {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/extension/runtime-095");
    let row = std::fs::read_to_string(root.join(format!("{label}.row.tsv"))).unwrap();
    let hex = row.trim().split('|').nth(2).unwrap();
    let bytes = (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap())
        .collect();
    (
        bytes,
        std::fs::read_to_string(root.join(format!("{label}.info.txt"))).unwrap(),
    )
}

fn property(info: &str, name: &str) -> String {
    info.lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            (key.trim() == name).then(|| value.trim().trim_matches('"').to_owned())
        })
        .unwrap()
}

#[test]
fn native_mode_profile_hash_and_version_are_kept_separate() {
    for label in [
        "safe-true",
        "safe-false",
        "ascii-profile",
        "unicode-profile",
        "versioned-unicode-profile",
        "mapping-false-profile",
        "legacy-a1-profile",
        "unsafe-protection-disabled",
    ] {
        let (bytes, info) = native(label);
        let parsed = parse_extension_info(&bytes).unwrap();
        assert_eq!(
            parsed.safe_mode,
            Some(property(&info, "safe-mode") == "yes"),
            "{label}"
        );
        assert_eq!(
            parsed.security_profile_name,
            Some(property(&info, "security-profile-name")),
            "{label}"
        );
        assert_eq!(
            parsed.hash_sum,
            Some(property(&info, "hash-sum")),
            "{label}"
        );
        assert_eq!(
            parsed.version,
            Some("0.9.5".to_owned()),
            "a profile must not replace the version: {label}"
        );
        assert_eq!(parsed.active, Some(true));
        assert_eq!(property(&info, "scope"), "infobase");
    }
}

#[test]
fn every_truncated_native_property_record_keeps_its_properties_unknown() {
    let (bytes, _) = native("unicode-profile");
    for end in 24..bytes.len() {
        let parsed = parse_extension_info(&bytes[..end]).unwrap();
        assert_eq!(parsed.safe_mode, None, "{end}");
        assert_eq!(parsed.security_profile_name, None, "{end}");
        assert_eq!(parsed.hash_sum, None, "{end}");
        assert_ne!(
            parsed.version.as_deref(),
            Some("Профиль095"),
            "profile is not a version: {end}"
        );
    }
}

#[test]
fn unknown_header_flags_invalid_unicode_and_fake_complete_tails_do_not_grant_properties() {
    let (bytes, _) = native("unicode-profile");
    for (offset, value) in [
        (24, 0xa3),
        (25, 0x98),
        (26, 7),
        (35, 0x83),
        (36, 0x98),
        (bytes.len() - 2, 0xa1),
    ] {
        let mut changed = bytes.clone();
        changed[offset] = value;
        let parsed = parse_extension_info(&changed).unwrap();
        assert_eq!(parsed.safe_mode, None, "{offset}");
        assert_eq!(parsed.security_profile_name, None, "{offset}");
        assert_eq!(parsed.hash_sum, None, "{offset}");
    }
    let mut changed = bytes.clone();
    changed[38] = 0;
    changed[39] = 0xd8;
    assert_eq!(
        parse_extension_info(&changed)
            .unwrap()
            .security_profile_name,
        None
    );
    let mut fake = bytes[..36].to_vec();
    fake.extend_from_slice(&[0x81, 0x20]);
    assert_eq!(parse_extension_info(&fake).unwrap().safe_mode, None);
}
