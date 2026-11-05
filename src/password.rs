//! Verification of a password against the platform's public
//! `StoredPasswordValue` (`СохраняемоеЗначениеПароля`).
//!
//! The algorithm is the one the maintainer supplied on 2026-10-26 as the
//! Python `verify_password` v0.2 function, covering the 8.3.27.2342
//! algorithm profiles:
//!
//! | Record | Profile | Algorithm on `candidate ‖ salt` | Digest bytes |
//! |---|---|---|---|
//! | `H1,H2` | [`PasswordProfile::LegacySha1`] | SHA-1, no salt | 20 |
//! | `H1,H2,1,salt` | [`PasswordProfile::Sha256`] | SHA-256 | 32 |
//! | `H1,H2,2,salt` | [`PasswordProfile::Sha512`] | SHA-512 | 64 |
//! | `H1,H2,3,salt` | [`PasswordProfile::Pbkdf2HmacSha256`] | PBKDF2-HMAC-SHA256, empty separate salt, 16,000 iterations | 64 |
//!
//! The salt is literal UTF-8 text appended to the candidate, never decoded.
//! Only `H1` decides the result; `H2` is an optional ASCII-uppercase
//! diagnostic. A value whose provenance is not one of the accepted
//! [`SourceProfile`]s is never interpreted. Verification performs no I/O,
//! and neither results nor their `Debug` output carry passwords or hashes.
//!
//! ```
//! use open_sdbl::password::{
//!     VerificationOptions, VerificationReason, VerificationStatus, verify_stored_password,
//! };
//!
//! let stored = "IaYx2h3Z5OS8l6IvFucwUQxq2E0=,pfQAOj19NcDRSmGz4CwWTv6dRSk=";
//! let result = verify_stored_password("PaSs123!", stored, &VerificationOptions::new());
//! assert_eq!(result.status(), VerificationStatus::Match);
//! assert_eq!(result.reason(), VerificationReason::PrimaryDigestMatch);
//! assert_eq!(result.verified(), Some(true));
//! ```

mod digest;

use std::fmt;

/// Candidate UTF-8 bytes accepted by default: 1 MiB.
pub const DEFAULT_CANDIDATE_BYTE_LIMIT: usize = 1_048_576;
/// Stored-value UTF-8 bytes accepted by default: 64 KiB.
pub const DEFAULT_STORED_VALUE_BYTE_LIMIT: usize = 65_536;
/// The fixed bound on candidate plus salt bytes for PBKDF2 (ID 3),
/// independent of the caller's input limits.
pub const PBKDF2_INPUT_BYTE_LIMIT: usize = 1_048_576;
/// PBKDF2 iterations of profile ID 3.
pub const PBKDF2_ITERATIONS: u32 = 16_000;

/// Where a stored value came from: an assertion of provenance, not an
/// algorithm selector and not something detected from the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SourceProfile {
    /// The public property of platform 8.3.27 (the default).
    #[default]
    PublicStoredValue8327,
    /// A synthetic test value in the 8.3.27 property format.
    SyntheticPublicStoredValue8327,
    /// The public property of an earlier platform.
    PublicStoredValueLegacy,
    /// A synthetic test value in the earlier property format.
    SyntheticPublicStoredValueLegacy,
    /// Any other provenance — a raw record blob, a guess, an unknown name.
    /// Such a value is never interpreted.
    Unconfirmed,
}

impl SourceProfile {
    /// The profile a name denotes; unknown names are [`Self::Unconfirmed`].
    #[must_use]
    pub fn from_name(name: &str) -> Self {
        match name {
            "public-stored-value-8.3.27" => Self::PublicStoredValue8327,
            "synthetic-public-stored-value-8.3.27" => Self::SyntheticPublicStoredValue8327,
            "public-stored-value-legacy" => Self::PublicStoredValueLegacy,
            "synthetic-public-stored-value-legacy" => Self::SyntheticPublicStoredValueLegacy,
            _ => Self::Unconfirmed,
        }
    }

    /// The name of the profile; `unconfirmed` for any other provenance.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::PublicStoredValue8327 => "public-stored-value-8.3.27",
            Self::SyntheticPublicStoredValue8327 => "synthetic-public-stored-value-8.3.27",
            Self::PublicStoredValueLegacy => "public-stored-value-legacy",
            Self::SyntheticPublicStoredValueLegacy => "synthetic-public-stored-value-legacy",
            Self::Unconfirmed => "unconfirmed",
        }
    }

    /// Whether the provenance is one of the four accepted profiles.
    #[must_use]
    pub const fn is_confirmed(self) -> bool {
        !matches!(self, Self::Unconfirmed)
    }

    /// Whether the value is synthetic test evidence. A synthetic value
    /// verifies the algorithm; it never authenticates a production client.
    #[must_use]
    pub const fn is_synthetic(self) -> bool {
        matches!(
            self,
            Self::SyntheticPublicStoredValue8327 | Self::SyntheticPublicStoredValueLegacy
        )
    }
}

/// Whether the user has a password, as far as the caller knows. This is
/// separate from whether standard authentication is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PasswordState {
    /// A password is set.
    Set,
    /// No password is set; nothing verifies.
    NotSet,
    /// Not known (the default).
    #[default]
    Unknown,
}

/// Whether the diagnostic second digest is compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SecondaryMode {
    /// `H2` is validated but not compared (the default).
    #[default]
    None,
    /// For an all-ASCII candidate, `H2` is compared with the digest of the
    /// candidate with `a`–`z` mapped to `A`–`Z`, salt unchanged. The
    /// comparison never decides the result.
    AsciiUpperDiagnostic,
}

/// How one verification is performed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationOptions {
    source: SourceProfile,
    password_state: PasswordState,
    secondary_mode: SecondaryMode,
    record_id: Option<String>,
    candidate_byte_limit: usize,
    stored_value_byte_limit: usize,
}

impl Default for VerificationOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl VerificationOptions {
    /// The defaults: source `public-stored-value-8.3.27`, password state
    /// unknown, secondary mode `NONE`, no record identifier, and the
    /// default byte limits.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            source: SourceProfile::PublicStoredValue8327,
            password_state: PasswordState::Unknown,
            secondary_mode: SecondaryMode::None,
            record_id: None,
            candidate_byte_limit: DEFAULT_CANDIDATE_BYTE_LIMIT,
            stored_value_byte_limit: DEFAULT_STORED_VALUE_BYTE_LIMIT,
        }
    }

    /// Sets the provenance of the stored value.
    #[must_use]
    pub const fn source(mut self, source: SourceProfile) -> Self {
        self.source = source;
        self
    }

    /// Sets the known password state.
    #[must_use]
    pub const fn password_state(mut self, state: PasswordState) -> Self {
        self.password_state = state;
        self
    }

    /// Sets the secondary mode.
    #[must_use]
    pub const fn secondary_mode(mut self, mode: SecondaryMode) -> Self {
        self.secondary_mode = mode;
        self
    }

    /// Sets a non-secret caller identifier echoed in the result.
    #[must_use]
    pub fn record_id(mut self, record_id: impl Into<String>) -> Self {
        self.record_id = Some(record_id.into());
        self
    }

    /// Sets the limit on candidate UTF-8 bytes.
    #[must_use]
    pub const fn candidate_byte_limit(mut self, limit: usize) -> Self {
        self.candidate_byte_limit = limit;
        self
    }

    /// Sets the limit on stored-value UTF-8 bytes.
    #[must_use]
    pub const fn stored_value_byte_limit(mut self, limit: usize) -> Self {
        self.stored_value_byte_limit = limit;
        self
    }

    /// The provenance in effect.
    #[must_use]
    pub const fn source_in_effect(&self) -> SourceProfile {
        self.source
    }

    /// The password state in effect.
    #[must_use]
    pub const fn password_state_in_effect(&self) -> PasswordState {
        self.password_state
    }

    /// The secondary mode in effect.
    #[must_use]
    pub const fn secondary_mode_in_effect(&self) -> SecondaryMode {
        self.secondary_mode
    }

    /// The caller identifier in effect.
    #[must_use]
    pub fn record_id_in_effect(&self) -> Option<&str> {
        self.record_id.as_deref()
    }
}

/// The outcome class of one verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum VerificationStatus {
    /// `H1` matches the candidate.
    Match,
    /// `H1` does not match the candidate.
    NoMatch,
    /// The caller stated that no password is set.
    NoPasswordConfigured,
    /// The value could not be judged: unconfirmed source, empty value, or
    /// an unavailable hash.
    Indeterminate,
    /// The record names an algorithm this verifier does not support.
    UnsupportedProfile,
    /// The record is malformed.
    InvalidRecord,
    /// An input or option is invalid.
    InvalidInput,
    /// An input exceeds its limit.
    LimitExceeded,
}

impl VerificationStatus {
    /// The status code, as the reference algorithm spells it.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Match => "MATCH",
            Self::NoMatch => "NO_MATCH",
            Self::NoPasswordConfigured => "NO_PASSWORD_CONFIGURED",
            Self::Indeterminate => "INDETERMINATE",
            Self::UnsupportedProfile => "UNSUPPORTED_PROFILE",
            Self::InvalidRecord => "INVALID_RECORD",
            Self::InvalidInput => "INVALID_INPUT",
            Self::LimitExceeded => "LIMIT_EXCEEDED",
        }
    }
}

/// Why a verification ended as it did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum VerificationReason {
    /// `MATCH`: the primary digest matches.
    PrimaryDigestMatch,
    /// `NO_MATCH`: the primary digest differs.
    PrimaryDigestMismatch,
    /// `NO_PASSWORD_CONFIGURED`: the caller stated the password is not set.
    ExplicitPasswordNotSet,
    /// `INDETERMINATE`: the provenance is not an accepted profile.
    SourceProfileUnconfirmed,
    /// `INDETERMINATE`: the stored property is empty.
    EmptyStoredValue,
    /// `INDETERMINATE`: the selected hash could not be computed.
    HashUnavailable,
    /// `UNSUPPORTED_PROFILE`: a four-field record with algorithm ID 0.
    NoncanonicalLegacyRecord,
    /// `UNSUPPORTED_PROFILE`: an algorithm ID other than 1–3.
    UnknownAlgorithmId,
    /// `INVALID_RECORD`: neither two nor four comma-separated fields.
    InvalidFieldCount,
    /// `INVALID_RECORD`: the algorithm ID is not ASCII decimal.
    InvalidAlgorithmId,
    /// `INVALID_RECORD`: a digest field is not canonical Base64.
    InvalidBase64,
    /// `INVALID_RECORD`: a digest has the wrong length for its algorithm.
    InvalidDigestLength,
    /// `INVALID_INPUT`: candidate or stored value is not a string.
    ExpectedStrings,
    /// `INVALID_INPUT`: source, secondary mode or record identifier is not
    /// a string.
    InvalidMetadata,
    /// `INVALID_INPUT`: the password state is not `true`, `false` or
    /// unknown.
    InvalidPasswordState,
    /// `INVALID_INPUT`: the secondary mode is not a known mode.
    InvalidSecondaryMode,
    /// `INVALID_INPUT`: a limit is not a nonnegative integer.
    InvalidLimit,
    /// `INVALID_INPUT`: an input is not valid Unicode.
    InvalidUnicode,
    /// `LIMIT_EXCEEDED`: an input exceeds its character or byte limit.
    InputTooLarge,
    /// `LIMIT_EXCEEDED`: candidate plus salt exceed the PBKDF2 bound.
    Pbkdf2InputTooLarge,
}

impl VerificationReason {
    /// The reason code, as the reference algorithm spells it.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::PrimaryDigestMatch => "PRIMARY_DIGEST_MATCH",
            Self::PrimaryDigestMismatch => "PRIMARY_DIGEST_MISMATCH",
            Self::ExplicitPasswordNotSet => "EXPLICIT_PASSWORD_NOT_SET",
            Self::SourceProfileUnconfirmed => "SOURCE_PROFILE_UNCONFIRMED",
            Self::EmptyStoredValue => "EMPTY_STORED_VALUE",
            Self::HashUnavailable => "HASH_UNAVAILABLE",
            Self::NoncanonicalLegacyRecord => "NONCANONICAL_LEGACY_RECORD",
            Self::UnknownAlgorithmId => "UNKNOWN_ALGORITHM_ID",
            Self::InvalidFieldCount => "INVALID_FIELD_COUNT",
            Self::InvalidAlgorithmId => "INVALID_ALGORITHM_ID",
            Self::InvalidBase64 => "INVALID_BASE64",
            Self::InvalidDigestLength => "INVALID_DIGEST_LENGTH",
            Self::ExpectedStrings => "EXPECTED_STRINGS",
            Self::InvalidMetadata => "INVALID_METADATA",
            Self::InvalidPasswordState => "INVALID_PASSWORD_STATE",
            Self::InvalidSecondaryMode => "INVALID_SECONDARY_MODE",
            Self::InvalidLimit => "INVALID_LIMIT",
            Self::InvalidUnicode => "INVALID_UNICODE",
            Self::InputTooLarge => "INPUT_TOO_LARGE",
            Self::Pbkdf2InputTooLarge => "PBKDF2_INPUT_TOO_LARGE",
        }
    }

    /// The status this reason belongs to.
    #[must_use]
    pub const fn status(self) -> VerificationStatus {
        match self {
            Self::PrimaryDigestMatch => VerificationStatus::Match,
            Self::PrimaryDigestMismatch => VerificationStatus::NoMatch,
            Self::ExplicitPasswordNotSet => VerificationStatus::NoPasswordConfigured,
            Self::SourceProfileUnconfirmed | Self::EmptyStoredValue | Self::HashUnavailable => {
                VerificationStatus::Indeterminate
            }
            Self::NoncanonicalLegacyRecord | Self::UnknownAlgorithmId => {
                VerificationStatus::UnsupportedProfile
            }
            Self::InvalidFieldCount
            | Self::InvalidAlgorithmId
            | Self::InvalidBase64
            | Self::InvalidDigestLength => VerificationStatus::InvalidRecord,
            Self::ExpectedStrings
            | Self::InvalidMetadata
            | Self::InvalidPasswordState
            | Self::InvalidSecondaryMode
            | Self::InvalidLimit
            | Self::InvalidUnicode => VerificationStatus::InvalidInput,
            Self::InputTooLarge | Self::Pbkdf2InputTooLarge => VerificationStatus::LimitExceeded,
        }
    }
}

/// A supported algorithm profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PasswordProfile {
    /// Two-field `H1,H2`: unsalted SHA-1.
    LegacySha1,
    /// Four-field ID 1: SHA-256.
    Sha256,
    /// Four-field ID 2: SHA-512.
    Sha512,
    /// Four-field ID 3: PBKDF2-HMAC-SHA256, 16,000 iterations, 64 bytes.
    Pbkdf2HmacSha256,
}

impl PasswordProfile {
    /// The profile code, as the reference algorithm spells it.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::LegacySha1 => "LEGACY_SHA1",
            Self::Sha256 => "SHA256",
            Self::Sha512 => "SHA512",
            Self::Pbkdf2HmacSha256 => "PBKDF2_HMAC_SHA256",
        }
    }

    /// The digest length in bytes.
    #[must_use]
    pub const fn digest_length(self) -> usize {
        match self {
            Self::LegacySha1 => 20,
            Self::Sha256 => 32,
            Self::Sha512 | Self::Pbkdf2HmacSha256 => 64,
        }
    }
}

/// What the diagnostic comparison of `H2` established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecondaryConsistency {
    /// `H2` was not compared (mode `NONE`, or an early return).
    NotChecked,
    /// Diagnostics were requested, but `H1` did not match or `H2` was not
    /// compared.
    NotEstablished,
    /// `H1` matched and `H2` agrees.
    Consistent,
    /// `H1` matched and `H2` disagrees; the result is still `MATCH`.
    Inconsistent,
}

impl SecondaryConsistency {
    /// The consistency code, as the reference algorithm spells it.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::NotChecked => "not_checked",
            Self::NotEstablished => "not_established",
            Self::Consistent => "consistent",
            Self::Inconsistent => "inconsistent",
        }
    }
}

/// The typed result of one verification. It carries no password and no
/// stored hash, so it can be logged as is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasswordVerification {
    reason: VerificationReason,
    profile: Option<PasswordProfile>,
    record_id: Option<String>,
    primary_match: Option<bool>,
    uppercase_match: Option<bool>,
    secondary_consistency: SecondaryConsistency,
}

impl PasswordVerification {
    fn early(reason: VerificationReason, record_id: Option<String>) -> Self {
        Self {
            reason,
            profile: None,
            record_id,
            primary_match: None,
            uppercase_match: None,
            secondary_consistency: SecondaryConsistency::NotChecked,
        }
    }

    fn with_profile(mut self, profile: PasswordProfile) -> Self {
        self.profile = Some(profile);
        self
    }

    /// The outcome class.
    #[must_use]
    pub const fn status(&self) -> VerificationStatus {
        self.reason.status()
    }

    /// Why the verification ended as it did.
    #[must_use]
    pub const fn reason(&self) -> VerificationReason {
        self.reason
    }

    /// `Some(true)` only for `MATCH`, `Some(false)` only for `NO_MATCH`,
    /// otherwise `None`.
    #[must_use]
    pub const fn verified(&self) -> Option<bool> {
        match self.reason.status() {
            VerificationStatus::Match => Some(true),
            VerificationStatus::NoMatch => Some(false),
            _ => None,
        }
    }

    /// The selected algorithm, once one was selected.
    #[must_use]
    pub const fn profile(&self) -> Option<PasswordProfile> {
        self.profile
    }

    /// The caller's non-secret identifier.
    #[must_use]
    pub fn record_id(&self) -> Option<&str> {
        self.record_id.as_deref()
    }

    /// Whether `H1` matched; `None` before the comparison.
    #[must_use]
    pub const fn primary_match(&self) -> Option<bool> {
        self.primary_match
    }

    /// Whether the diagnostic `H2` comparison matched; `None` when it did
    /// not run.
    #[must_use]
    pub const fn uppercase_match(&self) -> Option<bool> {
        self.uppercase_match
    }

    /// What the diagnostic comparison established.
    #[must_use]
    pub const fn secondary_consistency(&self) -> SecondaryConsistency {
        self.secondary_consistency
    }
}

impl fmt::Display for PasswordVerification {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} / {}",
            self.status().code(),
            self.reason.code()
        )?;
        if let Some(profile) = self.profile {
            write!(formatter, " ({})", profile.code())?;
        }
        Ok(())
    }
}

/// Verifies `candidate` against the public stored value `stored`.
///
/// Inputs are used exactly as given: nothing is trimmed, normalized,
/// case-folded or truncated at NUL.
#[must_use]
pub fn verify_stored_password(
    candidate: &str,
    stored: &str,
    options: &VerificationOptions,
) -> PasswordVerification {
    verify_with(candidate, stored, options, &BuiltInDigests)
}

/// Computes the digest a profile prescribes; `None` when unavailable.
pub(crate) trait DigestProvider {
    fn digest(&self, profile: PasswordProfile, input: &[u8]) -> Option<Vec<u8>>;
}

struct BuiltInDigests;

impl DigestProvider for BuiltInDigests {
    fn digest(&self, profile: PasswordProfile, input: &[u8]) -> Option<Vec<u8>> {
        Some(match profile {
            PasswordProfile::LegacySha1 => digest::sha1(input).to_vec(),
            PasswordProfile::Sha256 => digest::sha256(input).to_vec(),
            PasswordProfile::Sha512 => digest::sha512(input).to_vec(),
            PasswordProfile::Pbkdf2HmacSha256 => {
                let mut output = vec![0_u8; 64];
                digest::pbkdf2_hmac_sha256(input, b"", PBKDF2_ITERATIONS, &mut output);
                output
            }
        })
    }
}

/// Bytes wiped when dropped: candidate material never outlives its use.
struct Wiped(Vec<u8>);

impl Drop for Wiped {
    fn drop(&mut self) {
        for byte in &mut self.0 {
            // Volatile-free best effort: `black_box` keeps the writes.
            *byte = std::hint::black_box(0);
        }
        std::hint::black_box(&self.0);
    }
}

/// The checks every boundary applies after its own type checks: limits in
/// characters, then bytes, then the record itself.
pub(crate) fn verify_with(
    candidate: &str,
    stored: &str,
    options: &VerificationOptions,
    digests: &dyn DigestProvider,
) -> PasswordVerification {
    let record_id = options.record_id.clone();
    let early = |reason| PasswordVerification::early(reason, record_id.clone());
    // Characters first, as the reference counts them before encoding; a
    // character is at least one byte, so the byte check follows.
    if candidate.chars().count() > options.candidate_byte_limit
        || stored.chars().count() > options.stored_value_byte_limit
        || candidate.len() > options.candidate_byte_limit
        || stored.len() > options.stored_value_byte_limit
    {
        return early(VerificationReason::InputTooLarge);
    }
    verify_bounded(candidate, stored, options, digests)
}

fn verify_bounded(
    candidate: &str,
    stored: &str,
    options: &VerificationOptions,
    digests: &dyn DigestProvider,
) -> PasswordVerification {
    let record_id = options.record_id.clone();
    let early = |reason| PasswordVerification::early(reason, record_id.clone());
    if !options.source.is_confirmed() {
        return early(VerificationReason::SourceProfileUnconfirmed);
    }
    if options.password_state == PasswordState::NotSet {
        return early(VerificationReason::ExplicitPasswordNotSet);
    }
    if stored.is_empty() {
        return early(VerificationReason::EmptyStoredValue);
    }
    let fields = stored.split(',').collect::<Vec<_>>();
    let (profile, salt) = match fields.as_slice() {
        [_, _] => (PasswordProfile::LegacySha1, ""),
        [_, _, id, salt] => {
            if id.is_empty() || !id.bytes().all(|byte| byte.is_ascii_digit()) {
                return early(VerificationReason::InvalidAlgorithmId);
            }
            let normalized = id.trim_start_matches('0');
            let profile = match normalized {
                "" => return early(VerificationReason::NoncanonicalLegacyRecord),
                "1" => PasswordProfile::Sha256,
                "2" => PasswordProfile::Sha512,
                "3" => PasswordProfile::Pbkdf2HmacSha256,
                _ => return early(VerificationReason::UnknownAlgorithmId),
            };
            (profile, *salt)
        }
        _ => return early(VerificationReason::InvalidFieldCount),
    };
    let selected = |reason| early(reason).with_profile(profile);
    if profile == PasswordProfile::Pbkdf2HmacSha256
        && candidate.len().saturating_add(salt.len()) > PBKDF2_INPUT_BYTE_LIMIT
    {
        return selected(VerificationReason::Pbkdf2InputTooLarge);
    }
    let mut digests_stored = Vec::with_capacity(2);
    for field in &fields[..2] {
        let Some(decoded) = digest::base64_decode_canonical(field) else {
            return selected(VerificationReason::InvalidBase64);
        };
        if decoded.len() != profile.digest_length() {
            return selected(VerificationReason::InvalidDigestLength);
        }
        digests_stored.push(decoded);
    }
    let primary_input = Wiped([candidate.as_bytes(), salt.as_bytes()].concat());
    let Some(primary) = digests.digest(profile, &primary_input.0) else {
        return selected(VerificationReason::HashUnavailable);
    };
    let primary_match = digest::constant_time_equal(&primary, &digests_stored[0]);
    let (uppercase_match, consistency) = match options.secondary_mode {
        SecondaryMode::None => (None, SecondaryConsistency::NotChecked),
        SecondaryMode::AsciiUpperDiagnostic if candidate.is_ascii() => {
            let mut upper = Wiped(candidate.as_bytes().to_ascii_uppercase());
            upper.0.extend_from_slice(salt.as_bytes());
            let Some(secondary) = digests.digest(profile, &upper.0) else {
                return selected(VerificationReason::HashUnavailable);
            };
            let matched = digest::constant_time_equal(&secondary, &digests_stored[1]);
            let consistency = match (primary_match, matched) {
                (true, true) => SecondaryConsistency::Consistent,
                (true, false) => SecondaryConsistency::Inconsistent,
                (false, _) => SecondaryConsistency::NotEstablished,
            };
            (Some(matched), consistency)
        }
        SecondaryMode::AsciiUpperDiagnostic => (None, SecondaryConsistency::NotEstablished),
    };
    PasswordVerification {
        reason: if primary_match {
            VerificationReason::PrimaryDigestMatch
        } else {
            VerificationReason::PrimaryDigestMismatch
        },
        profile: Some(profile),
        record_id: options.record_id.clone(),
        primary_match: Some(primary_match),
        uppercase_match,
        secondary_consistency: consistency,
    }
}

/// One value at an untyped boundary, such as a value decoded from JSON or
/// a scripting bridge. [`verify_untyped`] checks the types the typed API
/// makes unrepresentable.
#[derive(Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum UntypedValue {
    /// The key is absent; the option takes its default.
    Missing,
    /// An explicit null.
    Null,
    /// A boolean.
    Bool(bool),
    /// An integer of any sign.
    Integer(i128),
    /// A string as UTF-16 code units, which may hold an isolated surrogate.
    Utf16(Vec<u16>),
    /// A well-formed string.
    Text(String),
}

impl fmt::Debug for UntypedValue {
    /// Says what kind of value it is; never its text, which may be a
    /// password.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Missing => "Missing",
            Self::Null => "Null",
            Self::Bool(_) => "Bool(..)",
            Self::Integer(_) => "Integer(..)",
            Self::Utf16(_) => "Utf16(<redacted>)",
            Self::Text(_) => "Text(<redacted>)",
        })
    }
}

/// The arguments of one verification at an untyped boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UntypedVerificationInput {
    /// The candidate password.
    pub candidate: UntypedValue,
    /// The public stored value.
    pub stored_value: UntypedValue,
    /// The source profile name; missing or null is the default.
    pub source_profile: UntypedValue,
    /// The password state: a boolean, or missing, null or `unknown`.
    pub password_is_set: UntypedValue,
    /// `NONE` or `ASCII_UPPER_DIAGNOSTIC`; missing or null is `NONE`.
    pub secondary_mode: UntypedValue,
    /// A non-secret identifier string, or missing/null.
    pub record_id: UntypedValue,
    /// The candidate limit in UTF-8 bytes; missing or null is the default.
    pub max_candidate_bytes: UntypedValue,
    /// The stored-value limit in UTF-8 bytes; missing or null is the
    /// default.
    pub max_stored_value_bytes: UntypedValue,
}

impl UntypedVerificationInput {
    /// Input with every option missing.
    #[must_use]
    pub const fn new(candidate: UntypedValue, stored_value: UntypedValue) -> Self {
        Self {
            candidate,
            stored_value,
            source_profile: UntypedValue::Missing,
            password_is_set: UntypedValue::Missing,
            secondary_mode: UntypedValue::Missing,
            record_id: UntypedValue::Missing,
            max_candidate_bytes: UntypedValue::Missing,
            max_stored_value_bytes: UntypedValue::Missing,
        }
    }
}

enum Textual {
    Valid(String),
    Invalid { characters: usize },
}

fn textual(value: &UntypedValue) -> Option<Textual> {
    match value {
        UntypedValue::Text(text) => Some(Textual::Valid(text.clone())),
        UntypedValue::Utf16(units) => Some(match String::from_utf16(units) {
            Ok(text) => Textual::Valid(text),
            Err(_) => Textual::Invalid {
                characters: char::decode_utf16(units.iter().copied()).count(),
            },
        }),
        _ => None,
    }
}

fn metadata_string(value: &UntypedValue) -> Result<Option<String>, ()> {
    match value {
        UntypedValue::Missing | UntypedValue::Null => Ok(None),
        UntypedValue::Text(text) => Ok(Some(text.clone())),
        UntypedValue::Utf16(units) => String::from_utf16(units).map(Some).map_err(|_| ()),
        _ => Err(()),
    }
}

fn limit(value: &UntypedValue, default: usize) -> Option<usize> {
    match value {
        UntypedValue::Missing | UntypedValue::Null => Some(default),
        UntypedValue::Integer(value) => usize::try_from(*value).ok(),
        _ => None,
    }
}

/// Verifies at an untyped boundary, checking in the reference order:
/// strings, metadata, password state, secondary mode, limits, character
/// counts, Unicode, byte counts — then the typed checks.
#[must_use]
pub fn verify_untyped(input: &UntypedVerificationInput) -> PasswordVerification {
    let (Some(candidate), Some(stored)) = (textual(&input.candidate), textual(&input.stored_value))
    else {
        return PasswordVerification::early(VerificationReason::ExpectedStrings, None);
    };
    let (Ok(source), Ok(mode), Ok(record_id)) = (
        metadata_string(&input.source_profile),
        metadata_string(&input.secondary_mode),
        metadata_string(&input.record_id),
    ) else {
        return PasswordVerification::early(VerificationReason::InvalidMetadata, None);
    };
    let early = |reason| PasswordVerification::early(reason, record_id.clone());
    let state = match &input.password_is_set {
        UntypedValue::Bool(true) => PasswordState::Set,
        UntypedValue::Bool(false) => PasswordState::NotSet,
        UntypedValue::Missing | UntypedValue::Null => PasswordState::Unknown,
        UntypedValue::Text(text) if text == "unknown" => PasswordState::Unknown,
        _ => return early(VerificationReason::InvalidPasswordState),
    };
    let mode = match mode.as_deref() {
        None | Some("NONE") => SecondaryMode::None,
        Some("ASCII_UPPER_DIAGNOSTIC") => SecondaryMode::AsciiUpperDiagnostic,
        Some(_) => return early(VerificationReason::InvalidSecondaryMode),
    };
    let (Some(candidate_limit), Some(stored_limit)) = (
        limit(&input.max_candidate_bytes, DEFAULT_CANDIDATE_BYTE_LIMIT),
        limit(
            &input.max_stored_value_bytes,
            DEFAULT_STORED_VALUE_BYTE_LIMIT,
        ),
    ) else {
        return early(VerificationReason::InvalidLimit);
    };
    let characters = |value: &Textual| match value {
        Textual::Valid(text) => text.chars().count(),
        Textual::Invalid { characters } => *characters,
    };
    if characters(&candidate) > candidate_limit || characters(&stored) > stored_limit {
        return early(VerificationReason::InputTooLarge);
    }
    let (Textual::Valid(candidate), Textual::Valid(stored)) = (candidate, stored) else {
        return early(VerificationReason::InvalidUnicode);
    };
    let candidate = Wiped(candidate.into_bytes());
    let candidate = std::str::from_utf8(&candidate.0).expect("the string was valid UTF-8");
    let mut options = VerificationOptions::new()
        .source(
            source
                .as_deref()
                .map_or_else(SourceProfile::default, SourceProfile::from_name),
        )
        .password_state(state)
        .secondary_mode(mode)
        .candidate_byte_limit(candidate_limit)
        .stored_value_byte_limit(stored_limit);
    options.record_id = record_id;
    verify_with(candidate, &stored, &options, &BuiltInDigests)
}

#[cfg(test)]
#[path = "password/tests.rs"]
mod tests;
