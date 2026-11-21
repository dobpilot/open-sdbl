## MODIFIED Requirements

### Requirement: Verify only confirmed supported stored-password values
The dependency-free, I/O-free core SHALL verify public `StoredPasswordValue`
using the supplied Python v0.2 algorithm: legacy SHA-1 and IDs 1–3.
`UserData::verify_password` SHALL verify against the stored password read
from the confirmed measured positions of the `v8users.Data` record (H1/H2
and the supported algorithm/salt suffix); when the record carries no stored
value at that position it SHALL return `INDETERMINATE` /
`SOURCE_PROFILE_UNCONFIRMED`. The stored value SHALL NOT appear in any
public getter, `Debug` output, error, or log.

#### Scenario: Public property is supplied
- **WHEN** a caller supplies a public `StoredPasswordValue` with the
  `public-stored-value-8.3.27` source profile
- **THEN** verification checks the value under its supported algorithm
  without reading or modifying a database

#### Scenario: Unconfirmed blob-derived value
- **WHEN** the source profile is `raw-blob` or any other unrecognized
  value
- **THEN** verification returns `INDETERMINATE` /
  `SOURCE_PROFILE_UNCONFIRMED` even if the text resembles valid `H1,H2`

#### Scenario: Synthetic evidence stays synthetic
- **WHEN** a fixture uses either accepted synthetic source profile
- **THEN** it can verify the algorithm but does not claim successful live
  authentication

#### Scenario: Record carries a stored password
- **WHEN** a `UserData` decoded from a record with a legacy `H1,H2` value
  verifies the password whose SHA-1 is `H1`
- **THEN** the result is `MATCH`, and a wrong password is `NO_MATCH`

#### Scenario: Record without a stored value
- **WHEN** a `UserData` record carries no digest fields at the measured
  position
- **THEN** `verify_password` returns `INDETERMINATE` /
  `SOURCE_PROFILE_UNCONFIRMED` for every candidate

#### Scenario: Stored value stays private
- **WHEN** a `UserData` holding a stored password is formatted for
  diagnostics
- **THEN** neither the hash nor the word "password" appears in its `Debug`
  output

## ADDED Requirements

### Requirement: Decode the measured extended v8users password suffix
For the measured extended record shapes, the decoder SHALL combine H1/H2
with the explicit algorithm ID and literal salt in the six-field suffix
after the counted optional string block. It SHALL validate the complete
shape and counters, keep the value private and never infer an algorithm
from digest length or fall back to legacy backup fields. Incomplete or
unknown extended shapes SHALL not authenticate.

#### Scenario: Native SHA-256 record
- **WHEN** the user record has the measured five-string optional block,
  suffix algorithm 1 and a salt string
- **THEN** verification uses SHA-256 over the candidate plus that literal
  salt, including when the salt text looks like Base64

#### Scenario: Native SHA-512 record shape
- **WHEN** the record has the measured two-string optional block and
  suffix algorithm 2
- **THEN** H1/H2, ID 2 and its literal salt are passed to the existing
  SHA-512 verifier without using legacy backup digests

#### Scenario: Malformed extended record
- **WHEN** optional counts, field types, suffix length or fixed flags are
  inconsistent, or the algorithm is unsupported
- **THEN** login fails with the ordinary authentication refusal
