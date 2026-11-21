## MODIFIED Requirements

### Requirement: Return typed verification outcomes without credentials
Results SHALL carry status, reason, profile, optional non-secret
`record_id`, `primary_match`, `uppercase_match`, `secondary_consistency`,
and `verified` (true only for `MATCH`, false only for `NO_MATCH`, otherwise
unset). All eight statuses and reason codes in the design SHALL remain
distinct. Verification results, `Debug` and errors SHALL NOT expose passwords or stored hashes. An explicitly named sensitive diagnostic getter MAY expose a stored value to the gateway for operator-enabled sensitive diagnostics. Verification SHALL perform no I/O or
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
