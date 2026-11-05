use super::*;

const VECTORS: &str = include_str!("../../tests/fixtures/password/vectors.tsv");
const DEMO: &str = "IaYx2h3Z5OS8l6IvFucwUQxq2E0=,pfQAOj19NcDRSmGz4CwWTv6dRSk=";
const EMPTY_SHA1: &str = "2jmj7l5rSw0yVb/vlWAYkK/YBwk=";

fn unhex(text: &str) -> Vec<u8> {
    if text == "-" {
        return Vec::new();
    }
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn optional_bool(text: &str) -> Option<bool> {
    match text {
        "-" => None,
        "true" => Some(true),
        "false" => Some(false),
        other => panic!("not a boolean: {other}"),
    }
}

#[test]
fn synthetic_fixtures_match_the_independent_reference() {
    let mut count = 0;
    for line in VECTORS.lines().filter(|line| !line.starts_with('#')) {
        let fields = line.split('\t').collect::<Vec<_>>();
        let [
            name,
            source,
            state,
            mode,
            candidate,
            stored,
            status,
            reason,
            profile,
            primary,
            upper,
            consistency,
        ] = fields[..]
        else {
            panic!("malformed fixture line {line:?}");
        };
        let candidate = String::from_utf8(unhex(candidate)).expect("candidate is UTF-8");
        let stored = if stored == "-" { "" } else { stored };
        let options = VerificationOptions::new()
            .source(SourceProfile::from_name(source))
            .password_state(match state {
                "true" => PasswordState::Set,
                "false" => PasswordState::NotSet,
                _ => PasswordState::Unknown,
            })
            .secondary_mode(match mode {
                "ASCII_UPPER_DIAGNOSTIC" => SecondaryMode::AsciiUpperDiagnostic,
                _ => SecondaryMode::None,
            });
        let result = verify_stored_password(&candidate, stored, &options);
        assert_eq!(result.status().code(), status, "{name}");
        assert_eq!(result.reason().code(), reason, "{name}");
        assert_eq!(
            result.profile().map_or("-", PasswordProfile::code),
            profile,
            "{name}"
        );
        assert_eq!(result.primary_match(), optional_bool(primary), "{name}");
        assert_eq!(result.uppercase_match(), optional_bool(upper), "{name}");
        assert_eq!(result.secondary_consistency().code(), consistency, "{name}");
        count += 1;
    }
    assert!(count > 100, "only {count} fixtures read");
}

#[test]
fn defaults_are_the_documented_ones() {
    let options = VerificationOptions::new();
    assert_eq!(
        options.source_in_effect(),
        SourceProfile::PublicStoredValue8327
    );
    assert_eq!(options.password_state_in_effect(), PasswordState::Unknown);
    assert_eq!(options.secondary_mode_in_effect(), SecondaryMode::None);
    assert_eq!(options.record_id_in_effect(), None);
    assert_eq!(VerificationOptions::default(), options);
    assert_eq!(
        SourceProfile::default().name(),
        "public-stored-value-8.3.27"
    );
}

#[test]
fn source_profiles_assert_provenance_only() {
    for name in [
        "public-stored-value-8.3.27",
        "synthetic-public-stored-value-8.3.27",
        "public-stored-value-legacy",
        "synthetic-public-stored-value-legacy",
    ] {
        let source = SourceProfile::from_name(name);
        assert!(source.is_confirmed());
        assert_eq!(source.name(), name);
    }
    for name in ["raw-blob", "", "PUBLIC-STORED-VALUE-8.3.27", "v8users.Data"] {
        let source = SourceProfile::from_name(name);
        assert_eq!(source, SourceProfile::Unconfirmed, "{name}");
        // Even a text that looks exactly like a valid value is not read.
        let result =
            verify_stored_password("PaSs123!", DEMO, &VerificationOptions::new().source(source));
        assert_eq!(
            result.reason(),
            VerificationReason::SourceProfileUnconfirmed
        );
        assert_eq!(result.verified(), None);
    }
    assert!(SourceProfile::SyntheticPublicStoredValueLegacy.is_synthetic());
    assert!(!SourceProfile::PublicStoredValue8327.is_synthetic());
}

#[test]
fn utf8_byte_limit_counts_bytes_not_characters() {
    let result = verify_stored_password(
        "é",
        DEMO,
        &VerificationOptions::new().candidate_byte_limit(1),
    );
    assert_eq!(result.status(), VerificationStatus::LimitExceeded);
    assert_eq!(result.reason(), VerificationReason::InputTooLarge);
    let result = verify_stored_password(DEMO, DEMO, &VerificationOptions::new());
    assert_eq!(result.reason(), VerificationReason::PrimaryDigestMismatch);
    let result = verify_stored_password(
        "x",
        DEMO,
        &VerificationOptions::new().stored_value_byte_limit(DEMO.len() - 1),
    );
    assert_eq!(result.reason(), VerificationReason::InputTooLarge);
    // Exactly the limit is accepted.
    let result = verify_stored_password(
        "PaSs123!",
        DEMO,
        &VerificationOptions::new()
            .candidate_byte_limit(8)
            .stored_value_byte_limit(DEMO.len())
            .source(SourceProfile::SyntheticPublicStoredValueLegacy),
    );
    assert_eq!(result.status(), VerificationStatus::Match);
}

#[test]
fn validation_precedes_source_and_password_state() {
    let oversized = "a".repeat(DEFAULT_CANDIDATE_BYTE_LIMIT + 1);
    let result = verify_stored_password(
        &oversized,
        DEMO,
        &VerificationOptions::new()
            .password_state(PasswordState::NotSet)
            .source(SourceProfile::Unconfirmed),
    );
    assert_eq!(result.reason(), VerificationReason::InputTooLarge);
    let result = verify_stored_password(
        "PaSs123!",
        DEMO,
        &VerificationOptions::new()
            .password_state(PasswordState::NotSet)
            .source(SourceProfile::Unconfirmed),
    );
    assert_eq!(
        result.reason(),
        VerificationReason::SourceProfileUnconfirmed
    );
    let result = verify_stored_password(
        "",
        &format!("{EMPTY_SHA1},{EMPTY_SHA1}"),
        &VerificationOptions::new().password_state(PasswordState::NotSet),
    );
    assert_eq!(result.status(), VerificationStatus::NoPasswordConfigured);
}

fn pbkdf2_record(salt: &str) -> String {
    let digest = digest::base64_encode(&[0_u8; 64]);
    format!("{digest},{digest},3,{salt}")
}

#[test]
fn pbkdf2_combined_limit_is_fixed_and_checked_before_digests() {
    let candidate = "a".repeat(PBKDF2_INPUT_BYTE_LIMIT);
    let raised = VerificationOptions::new()
        .candidate_byte_limit(4 * PBKDF2_INPUT_BYTE_LIMIT)
        .stored_value_byte_limit(4 * PBKDF2_INPUT_BYTE_LIMIT);
    for options in [VerificationOptions::new(), raised.clone()] {
        let result = verify_stored_password(&candidate, &pbkdf2_record("s"), &options);
        assert_eq!(result.status(), VerificationStatus::LimitExceeded);
        assert_eq!(result.reason(), VerificationReason::Pbkdf2InputTooLarge);
        assert_eq!(result.profile(), Some(PasswordProfile::Pbkdf2HmacSha256));
        assert_eq!(result.primary_match(), None);
        // Before the digests are looked at: malformed digests do not win.
        let result = verify_stored_password(&candidate, "!,!,3,s", &options);
        assert_eq!(result.reason(), VerificationReason::Pbkdf2InputTooLarge);
    }
    // A raised caller limit does not raise the PBKDF2 bound.
    let larger = "a".repeat(2 * PBKDF2_INPUT_BYTE_LIMIT);
    let result = verify_stored_password(&larger, &pbkdf2_record(""), &raised);
    assert_eq!(result.reason(), VerificationReason::Pbkdf2InputTooLarge);
    // The salt alone can push the input over the bound.
    let salt = "s".repeat(PBKDF2_INPUT_BYTE_LIMIT);
    let result = verify_stored_password("x", &pbkdf2_record(&salt), &raised);
    assert_eq!(result.reason(), VerificationReason::Pbkdf2InputTooLarge);
}

#[test]
fn pbkdf2_exact_bound_proceeds_to_digest_validation() {
    let candidate = "a".repeat(PBKDF2_INPUT_BYTE_LIMIT - 1);
    let result = verify_stored_password(&candidate, "!,!,3,s", &VerificationOptions::new());
    assert_eq!(result.reason(), VerificationReason::InvalidBase64);
    assert_eq!(result.profile(), Some(PasswordProfile::Pbkdf2HmacSha256));
}

#[test]
fn other_algorithms_have_no_combined_limit() {
    let candidate = "a".repeat(PBKDF2_INPUT_BYTE_LIMIT);
    let digest = digest::base64_encode(&[0_u8; 32]);
    let result = verify_stored_password(
        &candidate,
        &format!("{digest},{digest},1,s"),
        &VerificationOptions::new(),
    );
    assert_eq!(result.reason(), VerificationReason::PrimaryDigestMismatch);
}

#[test]
fn pbkdf2_uses_concatenated_password_and_empty_separate_salt() {
    // H1 from `pbkdf2_hmac("sha256", b"PaSs123!abc123", b"", 16000, 64)`.
    let concatenated = digest::base64_encode(&{
        let mut output = [0_u8; 64];
        digest::pbkdf2_hmac_sha256(b"PaSs123!abc123", b"", 16_000, &mut output);
        output
    });
    let separate = digest::base64_encode(&{
        let mut output = [0_u8; 64];
        digest::pbkdf2_hmac_sha256(b"PaSs123!", b"abc123", 16_000, &mut output);
        output
    });
    let options = VerificationOptions::new();
    let result = verify_stored_password(
        "PaSs123!",
        &format!("{concatenated},{concatenated},3,abc123"),
        &options,
    );
    assert_eq!(result.status(), VerificationStatus::Match);
    let result = verify_stored_password(
        "PaSs123!",
        &format!("{separate},{separate},3,abc123"),
        &options,
    );
    assert_eq!(result.status(), VerificationStatus::NoMatch);
}

#[test]
fn h2_is_validated_even_without_diagnostics() {
    let h1 = DEMO.split(',').next().unwrap();
    let options =
        VerificationOptions::new().source(SourceProfile::SyntheticPublicStoredValueLegacy);
    let result = verify_stored_password("PaSs123!", &format!("{h1},!"), &options);
    assert_eq!(result.reason(), VerificationReason::InvalidBase64);
    assert_eq!(result.primary_match(), None);
    assert_eq!(result.verified(), None);
    let result = verify_stored_password(
        "PaSs123!",
        &format!("{h1},{}", digest::base64_encode(&[0; 19])),
        &options,
    );
    assert_eq!(result.reason(), VerificationReason::InvalidDigestLength);
    assert_eq!(result.profile(), Some(PasswordProfile::LegacySha1));
}

struct Unavailable;

impl DigestProvider for Unavailable {
    fn digest(&self, _: PasswordProfile, _: &[u8]) -> Option<Vec<u8>> {
        None
    }
}

#[test]
fn unavailable_hash_is_indeterminate_with_profile() {
    let options = VerificationOptions::new().secondary_mode(SecondaryMode::AsciiUpperDiagnostic);
    let result = verify_with("PaSs123!", DEMO, &options, &Unavailable);
    assert_eq!(result.status(), VerificationStatus::Indeterminate);
    assert_eq!(result.reason(), VerificationReason::HashUnavailable);
    assert_eq!(result.profile(), Some(PasswordProfile::LegacySha1));
    assert_eq!(result.primary_match(), None);
    assert_eq!(result.uppercase_match(), None);
    assert_eq!(result.verified(), None);
    assert_eq!(
        result.secondary_consistency(),
        SecondaryConsistency::NotChecked
    );
}

#[test]
fn every_reason_belongs_to_its_documented_status() {
    use VerificationReason as R;
    use VerificationStatus as S;
    let table = [
        (R::PrimaryDigestMatch, S::Match),
        (R::PrimaryDigestMismatch, S::NoMatch),
        (R::ExplicitPasswordNotSet, S::NoPasswordConfigured),
        (R::SourceProfileUnconfirmed, S::Indeterminate),
        (R::EmptyStoredValue, S::Indeterminate),
        (R::HashUnavailable, S::Indeterminate),
        (R::NoncanonicalLegacyRecord, S::UnsupportedProfile),
        (R::UnknownAlgorithmId, S::UnsupportedProfile),
        (R::InvalidFieldCount, S::InvalidRecord),
        (R::InvalidAlgorithmId, S::InvalidRecord),
        (R::InvalidBase64, S::InvalidRecord),
        (R::InvalidDigestLength, S::InvalidRecord),
        (R::ExpectedStrings, S::InvalidInput),
        (R::InvalidMetadata, S::InvalidInput),
        (R::InvalidPasswordState, S::InvalidInput),
        (R::InvalidSecondaryMode, S::InvalidInput),
        (R::InvalidLimit, S::InvalidInput),
        (R::InvalidUnicode, S::InvalidInput),
        (R::InputTooLarge, S::LimitExceeded),
        (R::Pbkdf2InputTooLarge, S::LimitExceeded),
    ];
    let mut codes = std::collections::HashSet::new();
    for (reason, status) in table {
        assert_eq!(reason.status(), status, "{}", reason.code());
        assert!(codes.insert(reason.code()));
    }
}

fn text(value: &str) -> UntypedValue {
    UntypedValue::Text(value.to_owned())
}

#[test]
fn untyped_boundary_checks_types_in_reference_order() {
    let base = || UntypedVerificationInput::new(text("PaSs123!"), text(DEMO));

    let mut input = base();
    input.candidate = UntypedValue::Integer(1);
    input.password_is_set = text("yes");
    let result = verify_untyped(&input);
    assert_eq!(result.reason(), VerificationReason::ExpectedStrings);

    for field in 0..3 {
        let mut input = base();
        input.record_id = text("synthetic-user-1");
        input.password_is_set = text("yes");
        match field {
            0 => input.record_id = UntypedValue::Integer(7),
            1 => input.source_profile = UntypedValue::Bool(true),
            _ => input.secondary_mode = UntypedValue::Integer(0),
        }
        let result = verify_untyped(&input);
        assert_eq!(result.reason(), VerificationReason::InvalidMetadata);
        assert_eq!(result.record_id(), None);
    }

    let mut input = base();
    input.password_is_set = text("yes");
    input.secondary_mode = text("UNICODE_UPPER");
    assert_eq!(
        verify_untyped(&input).reason(),
        VerificationReason::InvalidPasswordState
    );

    let mut input = base();
    input.secondary_mode = text("UNICODE_UPPER");
    input.max_candidate_bytes = UntypedValue::Integer(-1);
    assert_eq!(
        verify_untyped(&input).reason(),
        VerificationReason::InvalidSecondaryMode
    );

    for limit in [
        UntypedValue::Integer(-1),
        UntypedValue::Bool(true),
        text("10"),
    ] {
        let mut input = base();
        input.max_stored_value_bytes = limit;
        assert_eq!(
            verify_untyped(&input).reason(),
            VerificationReason::InvalidLimit
        );
    }
}

#[test]
fn untyped_boundary_rejects_isolated_surrogates_after_character_limits() {
    let surrogate = UntypedValue::Utf16(vec![u16::from(b'a'), 0xD800]);
    let mut input = UntypedVerificationInput::new(surrogate.clone(), text(DEMO));
    assert_eq!(
        verify_untyped(&input).reason(),
        VerificationReason::InvalidUnicode
    );
    // Character limits come first: two characters over a limit of one.
    input.max_candidate_bytes = UntypedValue::Integer(1);
    assert_eq!(
        verify_untyped(&input).reason(),
        VerificationReason::InputTooLarge
    );
    // A byte limit follows encoding.
    let mut input = UntypedVerificationInput::new(text("é"), text(DEMO));
    input.max_candidate_bytes = UntypedValue::Integer(1);
    assert_eq!(
        verify_untyped(&input).reason(),
        VerificationReason::InputTooLarge
    );
}

#[test]
fn untyped_boundary_verifies_like_the_typed_one() {
    let mut input = UntypedVerificationInput::new(text("PaSs123!"), text(DEMO));
    input.source_profile = text("synthetic-public-stored-value-legacy");
    input.secondary_mode = text("ASCII_UPPER_DIAGNOSTIC");
    input.password_is_set = text("unknown");
    input.record_id = text("synthetic-user-1");
    let result = verify_untyped(&input);
    assert_eq!(result.status(), VerificationStatus::Match);
    assert_eq!(result.uppercase_match(), Some(true));
    assert_eq!(result.record_id(), Some("synthetic-user-1"));
    input.password_is_set = UntypedValue::Bool(false);
    assert_eq!(
        verify_untyped(&input).reason(),
        VerificationReason::ExplicitPasswordNotSet
    );
    input.source_profile = UntypedValue::Missing;
    input.password_is_set = UntypedValue::Null;
    assert_eq!(verify_untyped(&input).status(), VerificationStatus::Match);
}

#[test]
fn record_id_is_echoed_not_derived() {
    let result = verify_stored_password(
        "PaSs123!",
        DEMO,
        &VerificationOptions::new().record_id("synthetic-user-1"),
    );
    assert_eq!(result.record_id(), Some("synthetic-user-1"));
    let result = verify_stored_password("PaSs123!", DEMO, &VerificationOptions::new());
    assert_eq!(result.record_id(), None);
}

#[test]
fn diagnostics_never_expose_credentials() {
    let options = VerificationOptions::new()
        .source(SourceProfile::SyntheticPublicStoredValueLegacy)
        .secondary_mode(SecondaryMode::AsciiUpperDiagnostic)
        .record_id("synthetic-user-1");
    let mut input = UntypedVerificationInput::new(text("PaSs123!"), text(DEMO));
    input.record_id = text("synthetic-user-1");
    let formatted = [
        format!("{:?}", verify_stored_password("PaSs123!", DEMO, &options)),
        verify_stored_password("PaSs123!", DEMO, &options).to_string(),
        format!("{:?}", verify_stored_password("PaSs123!", "!,!", &options)),
        format!("{options:?}"),
        format!("{input:?}"),
        format!("{:?}", verify_untyped(&input)),
    ];
    let (h1, h2) = DEMO.split_once(',').unwrap();
    for text in formatted {
        for secret in ["PaSs123!", "PASS123!", DEMO, h1, h2] {
            assert!(!text.contains(secret), "{text} exposes {secret}");
        }
    }
}
