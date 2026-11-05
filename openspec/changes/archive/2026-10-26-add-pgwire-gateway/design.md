# Design: PostgreSQL wire-protocol gateway for SDBL

## Decisions taken with the maintainer (2026-10-26)

| # | Question | Decision |
|---|----------|----------|
| 1 | What "libpq adapter" means | A server speaking PostgreSQL protocol v3 (pgwire gateway). Not a drop-in `libpq.so` (would need FFI/`unsafe` and the libpq ABI), not a PostgreSQL extension (would require installing code on the 1C server, against the read-only model). |
| 2 | Backends | PostgreSQL and MSSQL 1C bases; the client always sees PostgreSQL. |
| 3 | Identity | Startup `user` is the information-base user; its roles, rights and restrictions apply. The gateway reaches the DBMS with its own operator-supplied credentials. |
| 4 | Protocol scope | Simple + Extended Query, TLS, CancelRequest. |
| 5 | Password check | Confirmed `StoredPasswordValue`: unsalted legacy SHA-1 or the supplied 8.3.27.2342 reference's SHA-256 / SHA-512 / PBKDF2-HMAC-SHA256 profiles (IDs 1–3). H1 decides, H2 is diagnostic. TLS is required except for explicitly allowed insecure loopback. Reverse engineering is out of scope. |
| 6 | Placement | New library crate `open-sdbl-pgwire` with a hand-written codec + `open-sdbl serve` in the CLI. |
| 7 | Types | Native by `ColumnKind`, consistent with the 0.3.0 "no text casts" decision. |
| 8 | Non-SDBL chatter | Minimal local whitelist; no SQL pass-through ever. |

## Package boundary

```
psql / JDBC / DBeaver ──pgwire v3──▶ open-sdbl-pgwire ──▶ open-sdbl (compile)
                                           │
                                           └──▶ open-sdbl-db (DatabaseSession, access, cells) ──▶ PG / MSSQL
open-sdbl-cli: `serve` parses arguments, loads secrets and TLS files, builds
the config and calls open_sdbl_pgwire::serve(listener, config, shutdown).
```

- `open-sdbl` stays I/O-free and dependency-free. Parameter discovery and
  typed stored-password verification (including the `UserData` adapter)
  are pure operations. SHA-1, SHA-256, SHA-512, HMAC-SHA256 and
  PBKDF2-HMAC-SHA256 are implemented in the core with independent
  known-answer vectors; strict canonical Base64 decoding is implemented
  locally for each digest length. No crypto or encoding production
  dependency is added.
- `open-sdbl-pgwire` depends on `open-sdbl`, `open-sdbl-db`, `tokio`,
  `rustls`, `tokio-rustls` (already a transitive dependency through
  `tokio-postgres-rustls`), `zeroize`. It accepts an already-bound
  `TcpListener`, a `GatewayConfig` (connection description, `Credentials`,
  `Limits`, `ServerConfig` for TLS, operator session parameters, policy
  flags) and a shutdown future. It reads no files: certificates arrive as
  parsed rustls objects.
- `open-sdbl-cli` owns: argument parsing, reading the PEM files, passwords
  from env/`~/.pgpass` (existing mechanism), logging connection events to
  stderr, signal handling for graceful shutdown.

### Why a hand-written codec instead of the `pgwire` crate
The subset needed is small (≈20 message types), the framing is trivial, and
the state machine must be tightly bound to SDBL semantics (Parse cannot
compile, Describe needs a late compile, errors carry SDBL positions). The
`pgwire` crate brings its own abstractions, `async-trait`, extra TLS glue
and frequent breaking releases. A local codec is fuzzable with the existing
`fuzz/` workspace (new target `pgwire_frontend`).

## Shared state and per-connection state

- **Shared (one per process):** metadata snapshot and `QueryCompiler`
  (acquired once at start, refreshed on `SIGHUP` by the CLI / on an explicit
  call by library users), `AccessStore` with rights of all roles, user list
  from `v8users`, extension index. Wrapped in `Arc` and swapped atomically
  on refresh; connections in flight keep the old snapshot until their
  current statement ends.
- **Per client connection:** one `DatabaseSession` opened at
  authentication, because the user's session parameters are read through
  it, and reopened on the next statement when it was lost outside a
  transaction block (pool is a later optimisation; one-to-one keeps
  transaction and cancellation semantics simple), the authenticated
  `InfoBaseUser`, its
  derived `RestrictionStore` and session parameters (computed exactly like
  console `\as`, at authentication), a `TempTablesManager`, named prepared
  statements and portals.

## Authentication flow

1. `SSLRequest` → `S` and TLS handshake when a TLS config is present, `N`
   otherwise. `GSSENCRequest` → `N`.
2. `StartupMessage`: protocol 3.0 required (3.2 negotiated down via
   `NegotiateProtocolVersion`); `user` required; `database` ignored but
   echoed; `application_name` kept for logs.
3. If the stream is not TLS and `allow_insecure_loopback` is not set for a
   loopback peer → `ErrorResponse 28000` without asking for a password.
4. `AuthenticationCleartextPassword`; the password is held in zeroized
   memory, checked with `UserData::verify_password` against the user found
   by exact (case-insensitive, as the platform does) name. The adapter
   requires a confirmed source mapping for the user's public
   `StoredPasswordValue`; the current decoder drops password fields, so
   it does not by itself establish such a mapping. Only primary-digest
   `MATCH` for a production source and `standard authentication` on
   permits login. Unknown user, wrong password, unsupported/malformed
   value, absent password, unconfirmed source, or standard authentication
   off → identical `28P01` message (it names no user), every failure
   answered no sooner than `GatewayLimits::authentication_failure_delay`
   after the password arrived to avoid user enumeration. Synthetic source
   profiles are for tests only.
5. Restrictions are derived (see `query-repl` "Run allowed queries as a
   user"); derivation failures for single objects are logged, not fatal —
   those objects then compile to the expansion error at query time.
6. `AuthenticationOk`, `ParameterStatus` (`server_version` = `13.0
   (open-sdbl X.Y.Z)`, `server_encoding`/`client_encoding` = `UTF8`,
   `DateStyle` = `ISO, MDY`, `integer_datetimes` = `on`,
   `standard_conforming_strings` = `on`, `TimeZone` = `UTC`,
   `application_name`), `BackendKeyData` (random process id + 32-bit secret
   per connection), `ReadyForQuery I`.

A client asking for `client_encoding` other than UTF8 is refused at startup
(`22023`).

### Password verification — supplied v0.2 algorithm

The reference is the updated Python `verify_password` v0.2 algorithm
supplied by the maintainer on 2026-10-26, covering the supplied
8.3.27.2342 algorithm profiles. Its companion spec and JSON vectors were not
supplied; the contract is recorded here and in the `access-rights` delta
without depending on those files. This change adapts the verification
function, not its standalone Python CLI, filesystem handling, or exit
codes.

The verifier consumes the platform's public `StoredPasswordValue`
(`СохраняемоеЗначениеПароля`), not serialized `v8users.Data`. Accepted source
profiles are `public-stored-value-8.3.27` (default),
`synthetic-public-stored-value-8.3.27`, `public-stored-value-legacy`, and
`synthetic-public-stored-value-legacy`. These are provenance assertions,
not content detection or algorithm selectors: the legacy-named sources
also accept supported extended records. The `UserData`
adapter may forward only a value with an already-confirmed mapping to
that property and user. With no such mapping it returns
`INDETERMINATE` / `SOURCE_PROFILE_UNCONFIRMED`. We do not research blob
layouts, create probe users, dump records, guess field offsets, or scan
for Base64 strings in this change. Supporting these algorithms does not
establish raw `v8users.Data` password extraction or live authentication.

| Record | Profile | Algorithm on input bytes | Digest bytes |
|---|---|---|---|
| `H1,H2` (implicit ID 0, no salt) | `LEGACY_SHA1` | SHA-1 | 20 |
| `H1,H2,1,salt` | `SHA256` | SHA-256 | 32 |
| `H1,H2,2,salt` | `SHA512` | SHA-512 | 64 |
| `H1,H2,3,salt` | `PBKDF2_HMAC_SHA256` | PBKDF2-HMAC-SHA256, separate salt empty, 16,000 iterations, 64 derived bytes | 64 |

For every extended profile, primary input is `UTF8(candidate) ||
UTF8(salt)`. Salt is literal text: `abc123` means the six bytes
`61 62 63 31 32 33`, not Base64-decoded content. Do not decode it as
Base64 or hex, change its case, trim or normalize it. Empty salt is
accepted. The legacy `H1,H2` format has no salt and hashes only the
candidate. For PBKDF2, the record salt is already appended to the password;
its separate salt argument is empty, not the record salt. Thus the
reference call for `PaSs123!` and `abc123` is
`pbkdf2_hmac("sha256", b"PaSs123!abc123", b"", 16000, dklen=64)`.

Processing order mirrors the supplied function:

1. Validate strings, metadata, password state (`true` / `false` /
   `unknown`), secondary mode, and nonnegative integer limits. Rust types
   may make invalid types and options unrepresentable. Default limits are
   1 MiB of candidate UTF-8 bytes and 64 KiB of stored-value UTF-8 bytes.
   Check character counts before encoding and byte counts afterwards;
   Rust strings already provide valid UTF-8. Do not trim, normalize,
   truncate at NUL, or otherwise transform either input.
2. Check source profile before interpreting the record. An unconfirmed
   source produces `INDETERMINATE` / `SOURCE_PROFILE_UNCONFIRMED`.
3. An explicit `password_is_set = false` produces
   `NO_PASSWORD_CONFIGURED` / `EXPLICIT_PASSWORD_NOT_SET`. Otherwise an
   empty property produces `INDETERMINATE` / `EMPTY_STORED_VALUE`.
   Password state defaults to `unknown`; it is separate from `EAuth`.
4. Split on literal commas preserving empty and trailing fields. Two
   fields select unsalted SHA-1. Four fields require a third field
   matching ASCII `[0-9]+`. Strip leading zeros (`000` becomes `0`)
   without integer conversion of arbitrarily long decimal strings.
   Four-field ID 0 produces `UNSUPPORTED_PROFILE` /
   `NONCANONICAL_LEGACY_RECORD`; normalized IDs outside 1–3 produce
   `UNSUPPORTED_PROFILE` / `UNKNOWN_ALGORITHM_ID`. Invalid decimal syntax
   produces `INVALID_RECORD` / `INVALID_ALGORITHM_ID`; other field counts
   produce `INVALID_RECORD` / `INVALID_FIELD_COUNT`. These returns have no
   profile and do not fall back to the first two fields.
5. Select the profile and digest size from the table. ID 3 enforces a
   fixed combined candidate-plus-salt UTF-8 limit of 1,048,576 bytes,
   independent of caller input limits. Exceeding it produces
   `LIMIT_EXCEEDED` / `PBKDF2_INPUT_TOO_LARGE` with profile
   `PBKDF2_HMAC_SHA256`, before validating digest fields; exactly the limit
   is allowed. The other algorithms have only the ordinary input limits.
6. Decode H1, then H2 as strict standard Base64; re-encoding must
   reproduce each field exactly (including pad bits). Both must decode
   to the selected digest length, even with diagnostics off.
   Distinguish `INVALID_BASE64` from `INVALID_DIGEST_LENGTH`; do not hash
   the candidate before both fields are valid.
7. Compute the selected algorithm over candidate bytes followed by literal
   salt bytes (no salt for legacy). Compare to H1 with a constant-time
   comparison for the selected digest length. Only H1 determines `MATCH` /
   `PRIMARY_DIGEST_MATCH` or `NO_MATCH` / `PRIMARY_DIGEST_MISMATCH`.
   An unavailable hash produces `INDETERMINATE` / `HASH_UNAVAILABLE` with
   the selected profile and no match assertions.
8. Secondary mode defaults to `NONE`: `uppercase_match = None`,
   consistency `not_checked`. Optional `ASCII_UPPER_DIAGNOSTIC` sets
   consistency to `not_established` and, only for all-ASCII candidates,
   compares the selected algorithm over `ascii_upper(candidate_bytes) ||
   salt_bytes` to H2, mapping only candidate `a`–`z` to `A`–`Z`. Salt bytes
   remain unchanged. The same PBKDF2 parameters apply to H1 and H2.
   No Unicode uppercase emulation is attempted.
   Consistency becomes `consistent` or `inconsistent` only if H1 matched
   and H2 was compared. H2 never authorizes a password or rejects a valid
   primary match. The gateway uses mode `NONE`.

The typed result carries `status`, `verified`, `primary_match`,
`uppercase_match`, `secondary_consistency`, `reason`, `profile`, and an
optional caller-provided non-secret `record_id`; a JSON API is not
required. `verified` is `Some(true)` only for `MATCH`, `Some(false)` only
for `NO_MATCH`, otherwise `None`. Early failures have no match assertions;
profile is `None` until a supported algorithm is selected. Digest errors,
hash failures and the combined PBKDF2 limit retain that profile. Invalid
metadata clears `record_id`. These distinctions stay internal to authentication;
all failures map to the same client-facing `28P01`.

| Status | Reasons |
|---|---|
| `MATCH` | `PRIMARY_DIGEST_MATCH` |
| `NO_MATCH` | `PRIMARY_DIGEST_MISMATCH` |
| `NO_PASSWORD_CONFIGURED` | `EXPLICIT_PASSWORD_NOT_SET` |
| `INDETERMINATE` | `SOURCE_PROFILE_UNCONFIRMED`, `EMPTY_STORED_VALUE`, `HASH_UNAVAILABLE` |
| `UNSUPPORTED_PROFILE` | `NONCANONICAL_LEGACY_RECORD`, `UNKNOWN_ALGORITHM_ID` |
| `INVALID_RECORD` | `INVALID_FIELD_COUNT`, `INVALID_ALGORITHM_ID`, `INVALID_BASE64`, `INVALID_DIGEST_LENGTH` |
| `INVALID_INPUT` | `EXPECTED_STRINGS`, `INVALID_METADATA`, `INVALID_PASSWORD_STATE`, `INVALID_SECONDARY_MODE`, `INVALID_LIMIT`, `INVALID_UNICODE` |
| `LIMIT_EXCEEDED` | `INPUT_TOO_LARGE`, `PBKDF2_INPUT_TOO_LARGE` |

Results, public getters, `Debug`, errors and logs never expose passwords
or stored hashes. An explicit empty-password digest is verifiable:
`H1 = H2 = 2jmj7l5rSw0yVb/vlWAYkK/YBwk=`. This differs from an empty
property and from explicit `password_is_set = false`, neither of which
is an authentication success under this contract.

The supplied synthetic demo is
`IaYx2h3Z5OS8l6IvFucwUQxq2E0=,pfQAOj19NcDRSmGz4CwWTv6dRSk=`.
For `PaSs123!`, H1 matches and the ASCII H2 diagnostic matches. For
`PASS123!`, only H2 matches, so verification fails. Replacing H2 with the
empty-password digest keeps `PaSs123!` a primary `MATCH` but diagnoses
`inconsistent`. Planned synthetic fixtures also cover non-ASCII UTF-8, embedded
NUL, unchanged whitespace, canonical padding, invalid H2, trailing commas,
four-field algorithms with literal salt `abc123`, Base64-looking and empty
salts, leading-zero and very long IDs, source provenance, digest sizes,
PBKDF2 parameters and combined limit, and validation precedence. Fixtures
for IDs 1–3 will be synthetic known-answer cases generated from the supplied
algorithm; they do not imply that live authentication was exercised.

## Query flow

### Simple Query
Text → whitelist check → otherwise `prepare_with_options(text, Restricted)`
→ derived restrictions for the requested targets → `check_user_field_access`
→ `compile_batch` with session parameters and the connection
`TempTablesManager` → for each `CompiledQuery` of the batch that returns
rows: `RowDescription`, `DataRow`s streamed from `query_each`,
`CommandComplete SELECT n`; temp-table statements (`ПОМЕСТИТЬ`,
`УНИЧТОЖИТЬ`) answer `CommandComplete SELECT 0` / `DROP TABLE`. A text with
`&Имя` that has no value → error `42P02` naming the parameter. Then
`ReadyForQuery`.

### Extended Query and `$n` ↔ `&Имя`
- **Parse**: whitelist check, else `prepare`. Parameter slots are the names
  returned by `Prepared::parameter_names()` in first-occurrence order:
  `$1` is the first distinct `&Имя`, `$2` the second, and so on. A statement
  text containing literal `$n` tokens is an SDBL lexing error. The declared
  parameter OIDs of Parse are kept (0 = unspecified).
- **Describe statement**: `ParameterDescription` with the declared OIDs,
  unspecified → `text` (25); `RowDescription` requires a compile, so it is
  produced by compiling with every parameter set to SQL `NULL` (column kinds
  do not depend on values except for a bare `&Имя` projection, which is
  described as `text`). If that compile fails, `NoData` is sent and the
  error surfaces at Execute.
- **Bind**: decode each value by its OID and format into `ParameterValue`:
  `bool`→Boolean; `int2/int4/int8/numeric/float8`→Number (float8 only if it
  round-trips exactly); `text/varchar/bpchar/unknown`→String;
  `timestamp/timestamptz/date`→Date (timestamptz converted to UTC);
  `bytea`→Binary, whose 16-byte form the compiler reads as a bare `RRRef`
  reference constant (and 20 bytes as `RTRef ‖ RRRef`), because a
  positional value names no metadata object; `uuid`→Uuid; one-dimensional
  arrays of the above→List (for `В`). NULL→Null. Anything else → `42804`.
  Then compile; compile diagnostics are reported here.
- **Execute** with `max_rows`: stream through `query_each`, suspend with
  `PortalSuspended` at the limit. Because the database layer reads inside
  one read-only transaction per call, a suspended portal keeps the remaining
  rows buffered up to `Limits`-based ceiling (new field
  `portal_buffer_rows`); exceeding it fails the portal with `54000` rather
  than re-running the query.
- **Sync** ends the implicit transaction; errors skip to Sync as the
  protocol requires.
- A projected tabular section is answered by a second statement
  (`CompiledQuery::nested`); Simple Query sends it as a further result
  set, while a portal carries one result, so Bind refuses such a statement
  with `0A000`.
- Deferred reference presentations are resolved after the rows are read,
  with lookups compiled in the statement's restricted mode and the user's
  decisions for the referenced objects; a target the user may not read
  presents nothing.

### Transactions
`BEGIN`/`START TRANSACTION` open a gateway-side block: subsequent reads run
inside one `begin_read_only_transaction` so the client gets a consistent
snapshot; `COMMIT`/`END`/`ROLLBACK` close it. Status byte follows
(`I`/`T`/`E`). Anything that would write cannot be expressed in SDBL.

### Errors
`QueryDiagnostic` → `ErrorResponse` with `S ERROR`, `C` mapped from
`QueryDiagnosticKind` (syntax → `42601`, unknown name → `42703`/`42P01`,
missing parameter → `42P02`, restriction/rights → `42501`, others →
`XX000`), `M` the diagnostic message, `P` = 1-based **character** position
computed from the byte offset (the protocol counts characters), `H` hint
where the diagnostic has one. `DbError` → `08006` for a dead session,
`57014` for cancellation/timeout, `XX000` otherwise; the original DBMS
message is not forwarded verbatim in `M` when it may contain physical
table names, it goes to `D` (detail) only when the gateway runs with
`--verbose-errors`.

### Whitelist (exact match after trimming and case-folding)
| Client text | Answer |
|---|---|
| empty / only comments | `EmptyQueryResponse` |
| `SET name = value` / `SET name TO value` / `RESET …` | `CommandComplete SET`; `client_encoding` other than UTF8 → error; `application_name` stored and reported |
| `SHOW name` | one-row result with the startup `ParameterStatus` value, `transaction_isolation` = `read committed`, others → `42704` |
| `BEGIN`, `START TRANSACTION [READ ONLY]`, `COMMIT`, `END`, `ROLLBACK`, `ABORT` | see Transactions; `READ WRITE` → `25006` |
| `DISCARD ALL` | clears prepared statements, portals, temp tables |
| `SELECT version()` / `select current_schema()` / `SELECT 1` | fixed one-row answers |
Everything else is SDBL.

### Cancellation
`CancelRequest` on a new connection with a matching `(pid, secret)` calls
`DatabaseSession::cancel_query` with that connection's `QueryCancellation`;
the running statement ends with `57014`. Unknown keys are ignored silently
as PostgreSQL does.

### Type mapping (result columns)
| ColumnKind | OID | text format | binary format |
|---|---|---|---|
| Reference (16 / 20 bytes) | `bytea` 17 | `\x…` lower hex | raw bytes |
| Binary | `bytea` 17 | `\x…` | raw |
| String | `text` 25 (`varchar` 1043 with typmod when length known) | as is | UTF-8 |
| Number | `numeric` 1700 with precision/scale typmod | decimal text with scale | numeric binary |
| Boolean | `bool` 16 | `t`/`f` | 1 byte |
| DateTime | `timestamp` 1114 | `YYYY-MM-DD HH:MM:SS` | int64 µs since 2000-01-01 |
| Uuid | `uuid` 2950 | canonical lower case | 16 bytes |
| Type / Undefined / Null / Unknown | `text` 25 / `bytea` | as the console renders | — |
MSSQL year offset is already removed by the compiler, so dates are 1C
calendar dates for both backends. `numeric` binary encoding is implemented
from the decimal string; tests cross-check with `tokio-postgres` decoding.

## Security considerations
- Default listen address `127.0.0.1:5433`; binding a non-loopback address
  without TLS is refused by the CLI.
- No SQL ever reaches the DBMS except the compiler's SELECT output and the
  existing fixed acquisition statements.
- `RestrictionMode::Restricted` + complete field check: a target or field
  that cannot be proven readable refuses the statement before SQL is
  generated.
- Operator-bound session parameters (e.g. data separator values) are fixed
  per gateway; the client cannot set session parameters. This keeps tenant
  isolation in the operator's hands.
- Limits: maximum message size (default 16 MiB, `08P01` beyond), maximum
  concurrent connections, startup/authentication timeout, idle timeout,
  per-statement `query_timeout` from `Limits`.
- Failed authentication attempts are logged by the CLI with peer address
  and user name, never with the password.

## Testing strategy
- Synthetic password-verification fixtures from the supplied v0.2 algorithm;
  SHA-1/SHA-256/SHA-512, HMAC/PBKDF2 and canonical Base64 unit vectors;
  no database access or reverse
  engineering for password verification. Tests distinguish algorithm
  verification from acquisition and live authentication.
- Codec unit tests per message, round-trip and malformed-frame tests; fuzz
  target `pgwire_frontend` in `fuzz/`.
- State-machine tests with an in-memory duplex stream and a fake executor
  trait (sealed inside the crate) — no database needed.
- Integration test using `tokio-postgres` as the client (dev-dependency)
  against the gateway with the fake executor: simple query, extended query
  with parameters of every OID class, describe, portal suspension, cancel,
  errors with positions, whitelist.
- Ignored live tests (like the MSSQL one) running `psql` against a real
  base on both backends.

## Open points to settle during implementation
- Whether an already-confirmed public-property source mapping exists for
  acquired users. If none exists, the `UserData` adapter remains
  indeterminate and gateway password authentication fails closed; this
  change does not add layout research to resolve that limitation.
- Whether metadata refresh should also be triggered automatically when the
  configuration version in `Config` changes; first version: manual only.
