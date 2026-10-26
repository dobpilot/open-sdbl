# Tasks

## 0. Gate
- [x] `openspec validate add-pgwire-gateway --strict` passes before any code.
- [x] Confirm `tokio-rustls` is already in `cargo tree` of the workspace (0.26.5 via `tokio-postgres-rustls`).

## 1. Measurements (blocks section 2)
- [ ] On an 8.2-layout and a modern probe base create users with known passwords (including empty and mixed case), dump `v8users.Data`, and record the field positions and hash scheme of the decoded record in design.md.
- [ ] Fix the `access-rights` scenarios with the measured values (fixtures under `tests/fixtures`).

## 2. Core additions (`open-sdbl`)
- [ ] Dependency-free SHA-1 and Base64 decode with FIPS 180-4 / RFC 4648 vectors.
- [ ] `UserData::verify_password` with constant-time comparison; `Debug` without hashes; fixture tests from section 1.
- [ ] `Prepared::parameter_names` with tests for order, case, literals, comments, batches, restriction-expanded session parameters.

## 3. Protocol codec (`open-sdbl-pgwire`)
- [ ] New workspace member, `#![forbid(unsafe_code)]`, `#![warn(missing_docs)]`.
- [ ] Frontend message decoder (startup, SSL/GSSENC/Cancel requests, Query, Parse, Bind, Describe, Execute, Close, Flush, Sync, Terminate, PasswordMessage) with size limit; backend message encoder.
- [ ] Round-trip and malformed-frame unit tests; fuzz target `pgwire_frontend` in `fuzz/`.

## 4. Connection state machine
- [ ] Startup, TLS negotiation, protocol negotiation, cleartext auth policy (TLS / explicit loopback), timing-uniform failures.
- [ ] Per-connection state: user, derived restrictions and session parameters, `TempTablesManager`, statements, portals, transaction status.
- [ ] Whitelist handling.
- [ ] Simple Query path; Extended Query path with `$k` ↔ `&Имя`, OID-driven `Bind` decoding, Describe via NULL-parameter compile, portal suspension with buffer ceiling.
- [ ] Diagnostic → `ErrorResponse` mapping with character positions; `DbError` mapping.
- [ ] Result type mapping and text/binary encoders for every `ColumnKind`.
- [ ] Cancel registry and `CancelRequest` handling.
- [ ] Limits: connections, startup timeout, idle timeout, portal buffer (new `Limits` field with default).
- [ ] In-memory duplex tests with a fake executor; `tokio-postgres` client integration tests (dev-dependency) covering every scenario of `pgwire-gateway`.

## 5. CLI
- [ ] `open-sdbl serve postgres|mssql` arguments, help text, non-loopback-without-TLS refusal.
- [ ] PEM loading, shared metadata/rights acquisition, `SIGHUP` refresh under `cfg(unix)`, graceful shutdown; Windows `cargo check` stays green.
- [ ] CLI tests for argument errors; ignored live tests with `psql` against PG and MSSQL bases.
- [ ] README (Russian) section: запуск шлюза, подключение psql/DBeaver, ограничения.

## 6. Finish
- [ ] Bump all workspace packages to 0.10.0.
- [ ] `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`.
- [ ] `openspec validate add-pgwire-gateway --strict`, then `openspec archive add-pgwire-gateway`.
