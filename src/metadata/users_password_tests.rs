//! The `UserData` adapter verifies against the stored password read from
//! the measured position of the `v8users.Data` record.

use super::*;
use crate::password::{
    PasswordState, SecondaryMode, VerificationOptions, VerificationReason, VerificationStatus,
};

/// SHA-1("123") base64, as it is stored in a legacy `H1,H2` record.
const SHA1_123: &str = "QL0AFWMIX8NRZTKeof9cXsvbvu8=";
/// SHA-1("") base64: a user whose password is the empty string.
const SHA1_EMPTY: &str = "2jmj7l5rSw0yVb/vlWAYkK/YBwk=";

#[path = "../../tests/support/password_record.rs"]
mod record;

fn vector(profile: &str) -> Vec<&'static str> {
    include_str!("../../tests/fixtures/password/vectors.tsv")
        .lines()
        .find(|l| l.starts_with(&format!("{profile}-match-public-stored-value-8.3.27\t")))
        .unwrap()
        .split('\t')
        .nth(5)
        .unwrap()
        .split(',')
        .collect()
}

#[test]
fn measured_extended_shapes_use_explicit_algorithm_and_literal_salt() {
    for tag in ["sha256", "sha512"] {
        let v = vector(tag);
        for optional in [2, 5] {
            let user = decode_user_data(&record::blob(v[0], v[1], v[2], v[3], optional)).unwrap();
            let matched = user.verify_password("PaSs123!", &VerificationOptions::new());
            assert_eq!(matched.status(), VerificationStatus::Match);
            assert_ne!(
                matched.profile(),
                Some(crate::password::PasswordProfile::LegacySha1)
            );
            for wrong in ["wrong", "PASS123!"] {
                assert_eq!(
                    user.verify_password(wrong, &VerificationOptions::new())
                        .status(),
                    VerificationStatus::NoMatch
                );
            }
            let debug = format!("{user:?}");
            for secret in [v[0], v[1], v[3]] {
                assert!(!debug.contains(secret));
            }
        }
    }
}

#[test]
fn extended_suffix_is_counted_after_separator_bindings() {
    let v = vector("sha256");
    let mut fields = record::fields(v[0], v[1], v[2], v[3], 5);
    fields[17] = "1".into();
    fields.insert(18, "{\"separator\",\"synthetic-value\"}".into());
    let user = decode_user_data(&record::encode(&fields)).unwrap();
    assert_eq!(
        user.verify_password("PaSs123!", &VerificationOptions::new())
            .status(),
        VerificationStatus::Match
    );
}

#[test]
fn malformed_extended_records_never_fall_back_to_legacy_or_backup_fields() {
    for mode in 0..10 {
        // A valid legacy digest must not authenticate when the record declares
        // an extended layout. Optional fields simulate legacy backup slots.
        let mut fields = record::fields(SHA1_123, SHA1_123, "1", "synthetic-salt", 5);
        fields[31] = format!("\"{SHA1_123}\"");
        fields[32] = format!("\"{SHA1_123}\"");
        match mode {
            0 => fields.truncate(41),
            1 => fields.push("0".into()),
            2 => fields[30] = "2".into(),
            3 => fields[30] = "3".into(),
            4 => fields[31] = "0".into(),
            5 => fields[36] = "\"1\"".into(),
            6 => fields[37] = "0".into(),
            7 => fields[38] = "1".into(),
            8 => fields[17] = "1".into(),
            _ => {}
        }
        let user = decode_user_data(&record::encode(&fields)).unwrap();
        assert_ne!(
            user.verify_password("123", &VerificationOptions::new())
                .status(),
            VerificationStatus::Match,
            "mode {mode}"
        );
    }
    let v = vector("sha256");
    for algorithm in ["0", "99", "invalid"] {
        let user = decode_user_data(&record::blob(v[0], v[1], algorithm, v[3], 5)).unwrap();
        assert_ne!(
            user.verify_password("PaSs123!", &VerificationOptions::new())
                .status(),
            VerificationStatus::Match
        );
    }
}

#[test]
fn packed_public_value_remains_supported() {
    let v = vector("sha256");
    let packed = v.join(",");
    let user = decode_user_data(&blob(&packed, "")).unwrap();
    assert_eq!(
        user.verify_password("PaSs123!", &VerificationOptions::new())
            .status(),
        VerificationStatus::Match
    );
}

/// Builds a `v8users.Data` blob from a record whose fields place `h1`/`h2`
/// at the measured offset (roles block at index 5, digests at 12/13), the
/// way the platform serializes it: a key-length byte, the XOR key, then
/// the UTF-8 BOM and the brace-serialized record XORed with the key.
fn blob(h1: &str, h2: &str) -> Vec<u8> {
    let record = format!(
        "{{3a442462-4e9b-4d59-8561-f445712ab12f,\"Тест\",\"\",\"Тест\",\
00000000-0000-0000-0000-000000000000,\
{{1,76702e9e-fa7a-4b98-befa-f9b37db2dae0}},\
00000000-0000-0000-0000-000000000000,1,1,,0,0,\"{h1}\",\"{h2}\",2}}"
    );
    let mut plain = vec![0xEF, 0xBB, 0xBF];
    plain.extend_from_slice(record.as_bytes());
    let key = b"\x11\x22\x33\x44\x55\x66\x77";
    let mut out = vec![key.len() as u8];
    out.extend_from_slice(key);
    out.extend(
        plain
            .iter()
            .enumerate()
            .map(|(index, byte)| byte ^ key[index % key.len()]),
    );
    out
}

#[test]
fn record_without_a_stored_value_stays_indeterminate() {
    let user = decode_user_data(&blob("", "")).expect("the synthetic record decodes");
    for candidate in ["", "123", SHA1_123] {
        let result = user.verify_password(candidate, &VerificationOptions::new());
        assert_eq!(result.status(), VerificationStatus::Indeterminate);
        assert_eq!(
            result.reason(),
            VerificationReason::SourceProfileUnconfirmed
        );
        assert_eq!(result.verified(), None);
        assert_eq!(result.profile(), None);
    }
}

#[test]
fn correct_password_matches_the_stored_value() {
    let user = decode_user_data(&blob(SHA1_123, SHA1_123)).expect("record decodes");
    let correct = user.verify_password("123", &VerificationOptions::new());
    assert_eq!(correct.status(), VerificationStatus::Match);
    assert_eq!(correct.verified(), Some(true));
    let wrong = user.verify_password("1234", &VerificationOptions::new());
    assert_eq!(wrong.status(), VerificationStatus::NoMatch);
    assert_eq!(wrong.verified(), Some(false));
}

#[test]
fn empty_password_user_accepts_the_empty_password() {
    let user = decode_user_data(&blob(SHA1_EMPTY, SHA1_EMPTY)).expect("record decodes");
    assert_eq!(
        user.verify_password("", &VerificationOptions::new())
            .status(),
        VerificationStatus::Match
    );
    assert_eq!(
        user.verify_password("x", &VerificationOptions::new())
            .status(),
        VerificationStatus::NoMatch
    );
}

#[test]
fn explicit_password_state_and_options_are_honoured() {
    let user = decode_user_data(&blob(SHA1_123, SHA1_123)).expect("record decodes");
    // The caller can still state the password is not set.
    let not_set = user.verify_password(
        "123",
        &VerificationOptions::new().password_state(PasswordState::NotSet),
    );
    assert_eq!(not_set.status(), VerificationStatus::NoPasswordConfigured);
    // The secondary diagnostic runs on the same stored value.
    let diagnostic = user.verify_password(
        "123",
        &VerificationOptions::new().secondary_mode(SecondaryMode::AsciiUpperDiagnostic),
    );
    assert_eq!(diagnostic.status(), VerificationStatus::Match);
    assert_eq!(diagnostic.uppercase_match(), Some(true));
}

#[test]
fn input_validation_still_runs_first() {
    let user = decode_user_data(&blob(SHA1_123, SHA1_123)).expect("record decodes");
    let result = user.verify_password(
        "пароль",
        &VerificationOptions::new().candidate_byte_limit(3),
    );
    assert_eq!(result.reason(), VerificationReason::InputTooLarge);
}

#[test]
fn debug_carries_no_password_material() {
    let user = decode_user_data(&blob(SHA1_123, SHA1_123)).expect("record decodes");
    let text = format!("{user:?}");
    assert!(!text.to_lowercase().contains("password"));
    assert!(!text.to_lowercase().contains("hash"));
    assert!(!text.contains(SHA1_123), "debug exposes the stored hash");
}
