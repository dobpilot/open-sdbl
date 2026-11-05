# Read the stored password from v8users so clients can log in

## Why
The pgwire gateway verifies passwords with a working SHA-1/256/512/PBKDF2
implementation, but nothing fed it the stored hash: `decode_user_data`
dropped the password fields of `v8users.Data` and
`UserData::verify_password` always answered `SOURCE_PROFILE_UNCONFIRMED`,
so every live login failed `28P01`. The field position was measured on an
authorized test base (`unf`, PostgreSQL, user `Администратор` with a known
password across all 13 `v8users` rows), which establishes the confirmed
source mapping the earlier change deliberately left open.

## What Changes
- `decode_user_data` reads the user's public `StoredPasswordValue` from
  the measured position of the record — the first nested list is the roles
  block, and the two digest fields `H1,H2` sit seven and eight fields after
  it — and keeps it on `UserData` as a private, `Debug`-redacted value.
- `UserData::verify_password` verifies the candidate against that stored
  value under the confirmed `public-stored-value-8.3.27` source; a record
  that carries no stored value still answers
  `INDETERMINATE` / `SOURCE_PROFILE_UNCONFIRMED`, so a user without a
  readable password fails closed.
- The gateway treats the `UserData` evidence as a confirmed production
  source, so a correct password now logs in; every non-`MATCH` outcome
  stays the identical `28P01`.
- The CLI startup line no longer claims every login fails closed.
- **BREAKING** (within the unreleased 0.10.0 cycle): the archived non-goal
  "does not discover raw `v8users.Data` password fields" is reversed, with
  the maintainer's authorization, for the legacy two-field layout measured
  on `unf`.

## Capabilities

### Modified Capabilities
- `access-rights`: "Verify only confirmed supported stored-password
  values" — `UserData::verify_password` now has a confirmed source mapping
  (the measured record position) and verifies against it, instead of
  always returning `SOURCE_PROFILE_UNCONFIRMED`.
- `pgwire-gateway`: "Authenticate the client as an information-base user" —
  a correct password against the stored value now permits login; the
  "no reverse engineering of password fields" clause is dropped.

## Impact
- Code: `src/metadata/users.rs` (capture the value, redact `Debug`);
  `UserData::verify_password`; the gateway `authenticate` source; the CLI
  startup message. No new dependency, no I/O added to the core, no change
  to generated SQL.
- Tests: synthetic-record unit tests for `decode`/`verify_password`; a
  gateway end-to-end login test through the `UserData` path.
- The extended native suffix is now measured for SHA-256/SHA-512 record
  shapes. Current SHA-256 authentication was verified read-only on `unf`.
  SHA-512 live login and native PBKDF2 records remain unmeasured; their
  cryptographic implementations are checked with synthetic vectors.

## Non-goals
- Changing hashing settings or passwords in the information base.
- Claiming live PBKDF2 compatibility or live SHA-512 login without a
  matching authorized runtime check.
- Reading any field other than the stored password from its measured
  position; the rest of the record is unchanged.
- Operating-system authentication of 1C users.
