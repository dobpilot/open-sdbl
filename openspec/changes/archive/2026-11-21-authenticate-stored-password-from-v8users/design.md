# Design: read the stored password from v8users.Data

## Measurement (authorized, 2026-10-26)

On the maintainer's `unf` PostgreSQL test base, user `Администратор` had a
known candidate. Decoding every `v8users.Data` blob (the existing XOR
step: first byte key length, then the key, then the UTF-8 BOM and the
brace-serialized record XORed with the cycled key) and checking the known
candidate against the primary slot pinned the layout, confirmed
across all 13 rows:

- The roles block is the first nested list in the record (`{N, <guid>…}`).
- `H1` is the field seven positions after the roles block, `H2` eight.
- Legacy format: `H1` = SHA-1(password) base64, `H2` = SHA-1(uppercase
  password). The SHA-1 of an empty input marks an empty password.
  A user with no password slot has both fields empty.
- The public `StoredPasswordValue` is therefore the string `"H1,H2"` — the
  verifier's existing `public-stored-value-legacy`/`8.3.27` two-field
  input.

Only structural facts are retained here; no live credential material is stored.

## Changes

- `decode_stored_password(fields)` returns `Some("H1,H2")` from the
  measured offset, `None` when both fields are empty, and — defensively —
  the first field whole when it already contains commas (an extended
  record packed into one field). `decode_user_data` stores the result in a
  new private `UserData::stored_password: Option<String>`.
- `UserData` loses `#[derive(Debug)]` for a manual `Debug` that omits the
  stored value, so no getter, `Debug`, error or log exposes a hash. The
  field is private; `#[non_exhaustive]` already blocks external
  construction.
- `UserData::verify_password` verifies against the stored value under
  `SourceProfile::PublicStoredValue8327` when present, else keeps the
  `Unconfirmed`/`INDETERMINATE` answer so a passwordless record fails
  closed. Caller password state, secondary mode, limits and `record_id`
  are preserved.
- The gateway's `authenticate` reports `PublicStoredValue8327` for
  `UserData` evidence, so a primary `MATCH` permits login; every other
  outcome remains the identical `28P01`.

## Extended suffix measurement (authorized, 2026-10-27)

After switching hashing on the same test base, the main H1/H2 slots still
held individual digests, with SHA-256 bytes rather than a packed four-field
string. The old adapter passed those slots as legacy SHA-1 and failed with
InvalidDigestLength. The existing cryptographic verifier was correct.

The measured extended records consist of the common prefix, separator
bindings counted at field 17, optional strings counted at `30 + bindings`,
then exactly six suffix fields: `[algorithm, literal salt, 0, 0, 0, string]`.
Two-string and five-string optional blocks were observed. With no separators,
the current SHA-256 record has 42 fields and algorithm/salt at 36/37; a
SHA-512-shaped record has 39 fields and algorithm/salt at 33/34. SHA-256
primary verification using the literal salt string matches the authorized
candidate. The salt is not Base64-decoded. No raw record, salt or live digest
is retained in fixtures, logs or this document.

The decoder validates counters, optional-string types, suffix flags and
length, then assembles `H1,H2,algorithm,salt` privately. Unknown extended
forms never fall back to the optional legacy backup digests. Full legacy
records must also retain a consistent counted shape, so a damaged count
cannot disguise an extended record as legacy. The previous packed-value
compatibility path remains for known short/legacy wrappers.

Core/gateway regressions use only synthetic credentials. An ignored read-only
test reads the selected live user through psql/pgpass with the candidate from
OPEN_SDBL_AUTH_IB_PASSWORD; its output includes only result/profile, never
credential material. The live SHA-256 record passes the ordinary UserData
gateway path; SHA-512 is covered structurally and with synthetic login tests.
The rebuilt release CLI also passed a real psql login and scalar SDBL query
against unf on a temporary loopback listener. A wrong password was refused;
the temporary process was stopped without touching the existing port 5433.

## Confidence and limits

- The legacy two-field layout is measured and trusted.
- SHA-256 live verification and the native SHA-256/SHA-512 record shapes are
  confirmed as described above. PBKDF2 native layout and live SHA-512 login
  remain unmeasured; algorithm vectors do not claim those live checks.
- No core I/O is added: the value is decoded from caller-provided bytes
  like every other `v8users` field.

## Testing

- Core: a synthetic blob builder places `H1,H2` at the measured offset;
  unit tests cover a correct password (`MATCH`), a wrong one (`NO_MATCH`),
  an empty-password record, an absent value (`INDETERMINATE`), option
  passthrough, input-limit precedence, and `Debug` redaction.
- Gateway: an end-to-end test logs in as a `UserData` user whose fixture
  record stores the empty-password SHA-1 and is refused with any non-empty
  password.
- Live checks are explicitly ignored by default and require the candidate
  through an environment variable. They read the user without modifying it.
