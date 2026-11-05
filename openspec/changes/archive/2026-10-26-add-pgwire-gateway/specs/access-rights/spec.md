## ADDED Requirements

### Requirement: Verify only confirmed supported stored-password values
The dependency-free, I/O-free core SHALL verify public `StoredPasswordValue`
using the supplied Python v0.2 algorithm: legacy SHA-1 and IDs 1–3.
`UserData::verify_password` SHALL use only an already-confirmed mapping
from a user's data to that public property; otherwise it SHALL return
`INDETERMINATE` / `SOURCE_PROFILE_UNCONFIRMED`. This change SHALL NOT
reverse engineer records, measure probe bases, discover password-field
positions, or infer provenance from hash-shaped text.

#### Scenario: Public property is supplied
- **WHEN** a caller supplies a public `StoredPasswordValue` with the
  `public-stored-value-8.3.27` source profile
- **THEN** verification checks the value under its supported algorithm
  without reading or modifying a database

#### Scenario: Unconfirmed blob-derived value
- **WHEN** the source profile is `raw-blob`, any other unrecognized value,
  or the `UserData` adapter has no confirmed source mapping
- **THEN** verification returns `INDETERMINATE` /
  `SOURCE_PROFILE_UNCONFIRMED` even if the text resembles valid `H1,H2`

#### Scenario: Synthetic evidence stays synthetic
- **WHEN** a fixture uses either accepted synthetic source profile
- **THEN** it can verify the algorithm but does not claim a known
  `v8users.Data` password-field position or successful live authentication

### Requirement: Use explicit verification profiles and defaults
Accepted sources SHALL be `public-stored-value-8.3.27`,
`synthetic-public-stored-value-8.3.27`, `public-stored-value-legacy`, and
`synthetic-public-stored-value-legacy`; others SHALL be unconfirmed.
Defaults SHALL be the first source, password state `unknown`, secondary
mode `NONE`, and unset `record_id`. Sources SHALL assert provenance, not
select algorithms. The `UserData` adapter SHALL NOT default unknown blob
mappings to confirmed sources.

#### Scenario: Default public-property verification
- **WHEN** a caller supplies the public property and candidate without
  overriding verification options
- **THEN** the source is `public-stored-value-8.3.27`, password state is
  `unknown`, secondary mode is `NONE`, and `record_id` is unset

#### Scenario: Legacy source names remain accepted
- **WHEN** either accepted legacy source name is used with a supported
  two-field or four-field value
- **THEN** verification selects the algorithm from the record format and
  ID, without limiting that source name to SHA-1

### Requirement: Preserve input and validate bounded verification options
Verification SHALL accept strings, source profile, password state,
secondary mode, optional non-secret string `record_id`, and nonnegative
integer byte limits. It SHALL validate types/options, character counts,
strict UTF-8 encoding, then byte counts before checking the source.
Defaults SHALL be 1,048,576 candidate bytes and 65,536 stored-value bytes.
Passwords SHALL NOT be trimmed, normalized, case-folded, or truncated.
Rust types MAY make invalid inputs unrepresentable.

#### Scenario: UTF-8 byte limit
- **WHEN** the candidate is `é` and its byte limit is 1
- **THEN** verification returns `LIMIT_EXCEEDED` / `INPUT_TOO_LARGE`
  although the candidate has only one character

#### Scenario: Embedded NUL is part of the password
- **WHEN** a candidate is `a\0b` and H1 is the SHA-1 digest of bytes
  `61 00 62` encoded in canonical Base64
- **THEN** verification hashes all three bytes without truncation

#### Scenario: Whitespace and Unicode are preserved
- **WHEN** candidates differ by a trailing newline, leading whitespace,
  case, or Unicode normalization form
- **THEN** verification hashes each exact UTF-8 byte sequence without
  transforming it

#### Scenario: Invalid input at an untyped boundary
- **WHEN** an adapter accepts a non-string candidate or a negative byte
  limit from an untyped caller
- **THEN** it returns `INVALID_INPUT` / `EXPECTED_STRINGS` or
  `INVALID_INPUT` / `INVALID_LIMIT`, respectively

### Requirement: Preserve input-validation failure reasons
Type/option checks SHALL precede size checks, with `INVALID_INPUT` reasons
`EXPECTED_STRINGS`, `INVALID_METADATA`, `INVALID_PASSWORD_STATE`,
`INVALID_SECONDARY_MODE`, then `INVALID_LIMIT`. Invalid metadata SHALL
clear `record_id`. Invalid Unicode SHALL yield `INVALID_INPUT` /
`INVALID_UNICODE` after character limits and before byte limits.
Exceeded limits SHALL yield `LIMIT_EXCEEDED` / `INPUT_TOO_LARGE` before
source checks or hashing. Limits SHALL be nonnegative integers.

#### Scenario: Invalid metadata clears the identifier
- **WHEN** an untyped adapter receives non-string `record_id`, source
  profile, or secondary mode
- **THEN** it returns `INVALID_INPUT` / `INVALID_METADATA` with no
  `record_id`

#### Scenario: Invalid state and mode are distinct
- **WHEN** an untyped caller supplies password state `yes`, secondary
  mode `UNICODE_UPPER`, or a negative limit in separate calls
- **THEN** the reasons are respectively `INVALID_PASSWORD_STATE`,
  `INVALID_SECONDARY_MODE`, and `INVALID_LIMIT`

#### Scenario: Invalid Unicode is rejected where representable
- **WHEN** an untyped boundary receives an isolated Unicode surrogate
  within the character-count limit
- **THEN** it returns `INVALID_INPUT` / `INVALID_UNICODE`

### Requirement: Distinguish unknown and explicitly absent passwords
After input validation, verification SHALL check source, explicit password
state, then empty stored value, in that order. A confirmed source with
state `false` SHALL yield `NO_PASSWORD_CONFIGURED` /
`EXPLICIT_PASSWORD_NOT_SET`. Empty stored content with state `true` or
`unknown` SHALL yield `INDETERMINATE` / `EMPTY_STORED_VALUE`. An empty
property, unknown state, or authentication flag SHALL NOT imply a stored
SHA-1 digest of the empty password.

#### Scenario: Explicitly unset password
- **WHEN** the source is confirmed and password state is `false`
- **THEN** even an empty candidate and a valid H1 yield
  `NO_PASSWORD_CONFIGURED` / `EXPLICIT_PASSWORD_NOT_SET`

#### Scenario: Empty property is unknown
- **WHEN** the source is confirmed, password state is `unknown`, and the
  stored value is empty
- **THEN** the result is `INDETERMINATE` / `EMPTY_STORED_VALUE`

#### Scenario: Validation precedes password state
- **WHEN** an oversized candidate is supplied with password state `false`
- **THEN** the result is `LIMIT_EXCEEDED` / `INPUT_TOO_LARGE`

#### Scenario: Source check precedes password state
- **WHEN** bounded valid inputs have an unconfirmed source and password
  state `false`
- **THEN** the result is `INDETERMINATE` / `SOURCE_PROFILE_UNCONFIRMED`

### Requirement: Parse stored-password fields without algorithm fallback
Verification SHALL split on literal commas, preserving empty/trailing
fields without trimming. Counts other than two or four SHALL yield
`INVALID_RECORD` / `INVALID_FIELD_COUNT`. Two fields SHALL select unsalted
SHA-1. Four fields SHALL be `H1,H2,algorithmId,salt`; the ID SHALL match
ASCII `[0-9]+`, else `INVALID_RECORD` / `INVALID_ALGORITHM_ID`. Supported
IDs SHALL select their algorithm without fallback to legacy or another ID.

#### Scenario: Extended profiles select their algorithm
- **WHEN** a four-field record has algorithm ID 1, 2, or 3
- **THEN** verification selects SHA-256, SHA-512, or PBKDF2-HMAC-SHA256,
  respectively, with the fourth field as literal salt

#### Scenario: Invalid algorithm identifier
- **WHEN** a four-field value has third field `+1`, an empty string, or
  non-ASCII digits
- **THEN** verification returns `INVALID_RECORD` / `INVALID_ALGORITHM_ID`

#### Scenario: Trailing separator is retained
- **WHEN** a valid two-field value is followed by one comma
- **THEN** verification sees three fields and returns `INVALID_RECORD` /
  `INVALID_FIELD_COUNT`

### Requirement: Normalize decimal IDs and reject unsupported profiles
Four-field IDs SHALL have leading zeros removed, with an all-zero ID
becoming `0`, without bounded-integer conversion of the original text.
ID 0 SHALL yield `UNSUPPORTED_PROFILE` / `NONCANONICAL_LEGACY_RECORD`;
IDs outside 1–3 SHALL yield `UNSUPPORTED_PROFILE` / `UNKNOWN_ALGORITHM_ID`.
These returns SHALL precede digest decoding and leave profile unset.
Unknown algorithms SHALL NOT fall back to the record's first two fields.

#### Scenario: Leading zeros do not change a supported ID
- **WHEN** algorithm IDs `0001`, `0002`, `0003`, or an ID containing
  thousands of leading zeros before `3` are supplied within input limits
- **THEN** they select IDs 1, 2, and 3 without integer conversion overflow

#### Scenario: Four-field legacy record is noncanonical
- **WHEN** a four-field value has algorithm ID `0` or `000`
- **THEN** the result is `UNSUPPORTED_PROFILE` /
  `NONCANONICAL_LEGACY_RECORD`, even if H1 would match SHA-1

#### Scenario: Unknown algorithm cannot reuse a valid H1
- **WHEN** algorithm ID is `4` or an arbitrarily long nonzero decimal ID
  within the stored-value limit
- **THEN** the result is `UNSUPPORTED_PROFILE` / `UNKNOWN_ALGORITHM_ID`,
  without integer conversion overflow or digest comparison

### Requirement: Preserve literal salt and unsalted legacy semantics
For four fields, salt SHALL be the exact UTF-8 bytes of the fourth field,
including case and whitespace; it SHALL NOT be Base64/hex decoded,
uppercased, or normalized. Algorithm input SHALL be candidate UTF-8 bytes
followed by salt bytes, without a delimiter. Empty salt SHALL be accepted.
The two-field `H1,H2` legacy format SHALL have no salt and SHALL hash only
candidate bytes.

#### Scenario: Salt abc123 is literal text
- **WHEN** candidate `PaSs123!` has salt field `abc123`
- **THEN** the primary algorithm input is the UTF-8 encoding of
  `PaSs123!abc123`, without Base64 decoding `abc123`

#### Scenario: Base64-looking salt and empty salt
- **WHEN** the salt is `YWJjMTIz` or empty
- **THEN** the salt bytes are respectively literal UTF-8 `YWJjMTIz` or
  zero bytes, rather than decoded `abc123` or missing-record errors

#### Scenario: Legacy record has no salt
- **WHEN** candidate `PaSs123!` is checked against a two-field value
- **THEN** the SHA-1 input is exactly `PaSs123!`, with no salt appended

### Requirement: Compute the selected algorithm with fixed parameters
Legacy SHA-1 SHALL use profile `LEGACY_SHA1` and 20 digest bytes; ID 1
SHALL use `SHA256` and 32 bytes; ID 2 SHALL use `SHA512` and 64 bytes.
ID 3 SHALL use profile `PBKDF2_HMAC_SHA256`, HMAC-SHA256, 16,000 iterations,
and 64 derived bytes. PBKDF2's password SHALL be the concatenated candidate
and literal record salt; its separate salt argument SHALL be empty.

#### Scenario: PBKDF2 does not use the record salt as a separate argument
- **WHEN** candidate `PaSs123!` has ID 3 and salt `abc123`
- **THEN** H1 is compared with
  `PBKDF2-HMAC-SHA256(UTF8("PaSs123!abc123"), salt=b"", iterations=16000, dkLen=64)`
  rather than PBKDF2 with password `PaSs123!` and separate salt `abc123`

### Requirement: Bound combined PBKDF2 input before digest decoding
After selecting ID 3 and setting its profile, verification SHALL enforce a
fixed 1,048,576-byte limit on candidate bytes plus literal salt bytes.
Exceeding it SHALL yield `LIMIT_EXCEEDED` / `PBKDF2_INPUT_TOO_LARGE` with
profile `PBKDF2_HMAC_SHA256` before decoding either digest. Raising caller
input limits SHALL NOT raise this limit. Other algorithms SHALL use the
ordinary candidate/stored-value limits without this combined-input check.

#### Scenario: Salt makes an otherwise valid candidate too large
- **WHEN** ID 3 has a 1,048,576-byte candidate and one-byte salt, with
  ordinary input limits satisfied
- **THEN** verification returns `PBKDF2_INPUT_TOO_LARGE` before digest
  decoding, including when the digest fields themselves are malformed

#### Scenario: Exact boundary is allowed
- **WHEN** candidate plus salt is exactly 1,048,576 bytes for ID 3
- **THEN** verification proceeds to digest validation

### Requirement: Validate both digests as canonical Base64
After algorithm selection and the PBKDF2 input check, verification SHALL
decode H1 then H2 as strict standard Base64 and require exact re-encoding
including pad bits. Invalid encoding SHALL yield `INVALID_RECORD` /
`INVALID_BASE64`; a canonical digest of the wrong algorithm-specific
length SHALL yield `INVALID_RECORD` / `INVALID_DIGEST_LENGTH`. Both fields
SHALL be validated before hashing, even when secondary checking is off.

#### Scenario: Malformed H2 cannot be bypassed
- **WHEN** H1 matches the candidate but H2 is `!`, with secondary mode
  `NONE`
- **THEN** verification returns `INVALID_RECORD` / `INVALID_BASE64`
  without reporting a primary match

#### Scenario: Noncanonical pad bits
- **WHEN** the first field is `2jmj7l5rSw0yVb/vlWAYkK/YBWl=`
  and the second is `2jmj7l5rSw0yVb/vlWAYkK/YBwk=`
- **THEN** verification rejects the first field as `INVALID_BASE64`
  even if a permissive decoder would produce the empty-password digest

#### Scenario: Canonical digest with wrong length
- **WHEN** either field decodes to a length other than 20 for legacy,
  32 for ID 1, or 64 for ID 2 or 3
- **THEN** verification returns `INVALID_RECORD` /
  `INVALID_DIGEST_LENGTH`

#### Scenario: Base64 must reproduce the original field
- **WHEN** a field contains whitespace, URL-safe substitutions, invalid
  characters, missing or extra padding, or noncanonical pad bits
- **THEN** verification returns `INVALID_RECORD` / `INVALID_BASE64`

### Requirement: Authorize only by the selected primary digest
Verification SHALL compare the selected algorithm's digest to H1 using a
constant-time comparison for its digest length. H1 alone SHALL determine `MATCH`
/ `PRIMARY_DIGEST_MATCH` or `NO_MATCH` / `PRIMARY_DIGEST_MISMATCH`.
H2 SHALL NOT authorize or override H1. An unavailable hash SHALL yield
`INDETERMINATE` / `HASH_UNAVAILABLE`, the selected profile, and no match
assertions. Whether the user may log in SHALL remain a separate
`InfoBaseUser` and gateway policy decision.

#### Scenario: Supplied synthetic demo
- **WHEN** the confirmed synthetic stored value is
  `IaYx2h3Z5OS8l6IvFucwUQxq2E0=,pfQAOj19NcDRSmGz4CwWTv6dRSk=`
  and the candidate is `PaSs123!`
- **THEN** the result is `MATCH` / `PRIMARY_DIGEST_MATCH`

#### Scenario: Uppercase variant cannot log in
- **WHEN** the same synthetic value is checked with `PASS123!` and
  `ASCII_UPPER_DIAGNOSTIC`
- **THEN** the result is `NO_MATCH`, even though the H2 comparison matches

#### Scenario: Explicit SHA-1 of an empty candidate
- **WHEN** the confirmed value is
  `2jmj7l5rSw0yVb/vlWAYkK/YBwk=,2jmj7l5rSw0yVb/vlWAYkK/YBwk=`,
  password state is `true` or `unknown`, and the candidate is empty
- **THEN** the result is `MATCH`, while a nonempty candidate yields
  `NO_MATCH`

### Requirement: Keep secondary checking diagnostic and ASCII-only
`NONE` SHALL leave `uppercase_match` unset and consistency `not_checked`.
`ASCII_UPPER_DIAGNOSTIC` SHALL compare the selected digest of uppercase
ASCII candidate bytes plus unchanged salt to H2 in constant time (map
only `a`–`z` to `A`–`Z`). Consistency SHALL be `not_established` unless H1
matches and H2 was compared, then `consistent` or `inconsistent`.
H2 SHALL NOT alter H1's result. Non-ASCII H2 checks SHALL be skipped.

#### Scenario: Both synthetic digests agree
- **WHEN** the supplied demo is checked with `PaSs123!` and
  `ASCII_UPPER_DIAGNOSTIC`
- **THEN** `primary_match` and `uppercase_match` are true and
  `secondary_consistency` is `consistent`

#### Scenario: Inconsistent H2 does not reject H1
- **WHEN** the demo's H2 is replaced by the canonical empty-password
  digest and the original candidate is checked with
  `ASCII_UPPER_DIAGNOSTIC`
- **THEN** the result is `MATCH`, `uppercase_match` is false, and
  `secondary_consistency` is `inconsistent`

#### Scenario: Non-ASCII secondary comparison is skipped
- **WHEN** a non-ASCII candidate is checked with `ASCII_UPPER_DIAGNOSTIC`
- **THEN** `uppercase_match` is unset, consistency is `not_established`,
  and H1 is still checked against the exact UTF-8 candidate followed by
  unchanged salt when present

#### Scenario: H2 match without H1 match proves no consistency
- **WHEN** H1 mismatches but the ASCII diagnostic comparison matches H2
- **THEN** the result is `NO_MATCH`, `uppercase_match` is true, and
  `secondary_consistency` remains `not_established`

#### Scenario: Uppercase diagnostics preserve salt in extended records
- **WHEN** candidate `PaSs123!`, salt `abc123`, and
  `ASCII_UPPER_DIAGNOSTIC` are used with ID 1, 2, or 3
- **THEN** the H2 algorithm input is UTF-8 `PASS123!abc123`, preserving
  the salt's lowercase letters; H1 still decides the result

#### Scenario: Non-ASCII salt does not disable ASCII diagnostics
- **WHEN** the candidate is ASCII `PaSs123!` but salt is `ёabc123`
- **THEN** the H2 diagnostic still runs on UTF-8 `PASS123!ёabc123`,
  preserving the salt exactly

### Requirement: Return typed verification outcomes without credentials
Results SHALL carry status, reason, profile, optional non-secret
`record_id`, `primary_match`, `uppercase_match`, `secondary_consistency`,
and `verified` (true only for `MATCH`, false only for `NO_MATCH`, otherwise
unset). All eight statuses and reason codes in the design SHALL remain
distinct. Results, public getters, `Debug`, errors, and logs SHALL NOT
expose passwords or stored hashes. Verification SHALL perform no I/O or
modify user/password records.

#### Scenario: Failure is distinct from a password mismatch
- **WHEN** H2 is malformed, the source is unconfirmed, or a profile is
  unsupported
- **THEN** `verified` and both match fields are unset, rather than false,
  and the typed status and reason identify the respective failure

#### Scenario: Early return has no match assertions
- **WHEN** verification returns before primary comparison
- **THEN** match fields are unset and consistency is `not_checked`;
  profile is unset until a supported algorithm is selected, then identifies
  that algorithm even for digest errors or the PBKDF2 input limit

#### Scenario: Credential material stays private
- **WHEN** the supplied demo is verified and its result, `UserData`, and
  any error are formatted for diagnostics
- **THEN** none exposes `PaSs123!`, the stored value, or either stored hash

#### Scenario: Caller identifier is retained safely
- **WHEN** a caller supplies non-secret `record_id` `synthetic-user-1`
- **THEN** the result retains that identifier without deriving an
  identifier from the password or hashes
