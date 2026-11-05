# Tasks

## 0. Gate
- [x] `openspec validate add-pgwire-gateway --strict` passes before any code.
- [x] Confirm `tokio-rustls` is already in `cargo tree` of the workspace (0.26.5 via `tokio-postgres-rustls`).

## 1. Stored-password verification contract and fixtures
- [x] Add self-contained synthetic fixtures under `tests/fixtures` from the supplied Python v0.2 algorithm for SHA-1, SHA-256, SHA-512, and PBKDF2-HMAC-SHA256: `PaSs123!`, wrong case, empty password, UTF-8, embedded NUL, and inconsistent H2. Record their synthetic provenance; no probe bases, dumps, or reverse engineering.
- [x] Document the already-confirmed source mapping available to `UserData`, if any; test `SOURCE_PROFILE_UNCONFIRMED` when none is available. Do not infer password-field positions or mark raw `v8users.Data` support as established by synthetic vectors.

## 2. Core additions (`open-sdbl`)
- [x] Dependency-free SHA-1, SHA-256, SHA-512, HMAC-SHA256 and PBKDF2-HMAC-SHA256 with independent known-answer vectors, plus strict canonical Base64 for 20/32/64-byte digests, including nonzero pad bits, invalid padding, whitespace, and wrong digest lengths.
- [x] Pure typed `StoredPasswordValue` verification and `UserData::verify_password` adapter with constant-time digest comparison, source-profile and password-state checks, input byte limits, and the result/status/reason contract in `access-rights`; fixture tests from section 1.
- [x] Test two-field SHA-1 and four-field ID 1–3 acceptance; leading-zero and arbitrarily long decimal IDs, unsupported four-field ID 0 and unknown IDs without fallback; all four accepted source profiles with default `public-stored-value-8.3.27`.
- [x] Test literal UTF-8 salt concatenation (including empty, mixed-case and Base64-looking salts), PBKDF2's empty separate salt / 16,000 iterations / 64 derived bytes, and the fixed 1 MiB combined-input limit before digest decoding, even when caller limits are raised.
- [x] Test ASCII-only H2 diagnostics with the selected algorithm and unchanged salt, exact UTF-8 preservation, `HASH_UNAVAILABLE`, validation precedence, and absence of credential material from results, `Debug`, errors, and logs.
- [x] `Prepared::parameter_names` with tests for order, case, literals, comments, batches, restriction-expanded session parameters.

## 3. Protocol codec (`open-sdbl-pgwire`)
- [x] New workspace member, `#![forbid(unsafe_code)]`, `#![warn(missing_docs)]`.
- [x] Frontend message decoder (startup, SSL/GSSENC/Cancel requests, Query, Parse, Bind, Describe, Execute, Close, Flush, Sync, Terminate, PasswordMessage) with size limit; backend message encoder.
- [x] Round-trip and malformed-frame unit tests; fuzz target `pgwire_frontend` in `fuzz/`.

## 4. Connection state machine
- [x] Startup, TLS negotiation, protocol negotiation, cleartext auth policy (TLS / explicit loopback), timing-uniform failures; only a confirmed-source primary `MATCH` for one of the four supported algorithms plus standard authentication permits login, all other verification outcomes map to identical `28P01` failures.
- [x] Per-connection state: user, derived restrictions and session parameters, `TempTablesManager`, statements, portals, transaction status.
- [x] Whitelist handling.
- [x] Simple Query path; Extended Query path with `$k` ↔ `&Имя`, OID-driven `Bind` decoding, Describe via NULL-parameter compile, portal suspension with buffer ceiling.
- [x] Diagnostic → `ErrorResponse` mapping with character positions; `DbError` mapping.
- [x] Result type mapping and text/binary encoders for every `ColumnKind`.
- [x] Cancel registry and `CancelRequest` handling.
- [x] Limits: connections, startup timeout, idle timeout, portal buffer (new `Limits` field with default).
- [x] In-memory duplex tests with a fake executor; `tokio-postgres` client integration tests (dev-dependency) covering every scenario of `pgwire-gateway`.

## 5. CLI
- [x] `open-sdbl serve postgres|mssql` arguments, help text, non-loopback-without-TLS refusal.
- [x] PEM loading, shared metadata/rights acquisition, `SIGHUP` refresh under `cfg(unix)`, graceful shutdown; Windows `cargo check` stays green.
- [x] CLI tests for argument errors; ignored live tests with `psql` against PG and MSSQL bases.
- [x] README (Russian) section: запуск шлюза, подключение psql/DBeaver, ограничения; legacy SHA-1 and ID 1–3 profiles from the supplied 8.3.27.2342 reference, confirmed-source requirement, no claim of raw `v8users.Data` password extraction or live authentication based on synthetic tests.

## 6. Finish
- [x] Bump all workspace packages to 0.10.0.
- [x] `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`.
- [x] `openspec validate add-pgwire-gateway --strict`, then `openspec archive add-pgwire-gateway`.
