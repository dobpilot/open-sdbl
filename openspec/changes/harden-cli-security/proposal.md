## Why

A security and reliability audit of the workspace (library plus the
`open-sdbl-cli` demo application) found that after the
`harden-core-reliability` campaign the remaining risk is concentrated in
the CLI and in two library API invariants:

1. **Terminal injection.** `escape_field` escapes only `\`, `\t`, `\r`,
   and `\n`, so ESC/OSC control sequences and Unicode bidi overrides
   stored in 1C table data flow raw into the operator's terminal — anyone
   with write access to business data can clear the screen, retitle the
   window, or abuse OSC 52 clipboard writes when an operator SELECTs the
   row.
2. **Transport security.** PostgreSQL connects exclusively with `NoTls`
   (no flag exists, `PGSSLMODE` is silently ignored); combined with
   `--socks5-proxy` the proxy operator has full MITM. The MSSQL TLS stack
   pins `rustls 0.21` (EOL) with three RUSTSEC advisories in
   `rustls-webpki 0.101.7`, two of them about accepting incorrect name
   constraints during certificate validation. `--trust-server-certificate`
   silently disables both chain and hostname verification.
3. **Read-only and session reliability.** On MSSQL `readonly(true)` only
   sets `ApplicationIntent` — it does not prevent writes, unlike the
   PostgreSQL path which opens a verified read-only transaction. No query
   after the handshake has a timeout; Ctrl-C during a query kills the
   process without cancelling the server-side query, leaving MSSQL
   transactions holding locks and the terminal in a modified state
   because RAII guards never run.
4. **Library invariants.** `MetadataSnapshot` exposes `pub` vectors while
   its private index stores numeric positions into them: mutating the
   vectors causes index-out-of-bounds panics inside
   `QueryCompiler::compile`, or worse, silently resolves a *different*
   object whose physical table then reaches generated SQL.
   `Prepared::compile` accepts an arbitrary snapshot, so presentation
   plans authorized against one snapshot can address different physical
   columns in another.
5. **Resource limits and hygiene.** The completion catalog materializes a
   cartesian product of reference aliases (hundreds of MB on real bases);
   the presentation cache computes its fallback before consulting the
   cache (caching nothing); the Config pipeline accumulates every decoded
   resource in memory; stdout printing panics on EPIPE; `.pgpass` is read
   with a stat-then-open TOCTOU window and passwords are never zeroized.

## What Changes

- Escape all control characters and bidirectional overrides in every
  terminal output path; bound printed rows and cell widths; route output
  through a writer that treats broken pipes as normal termination.
- Add TLS support for PostgreSQL with an `--sslmode` flag defaulting to
  full verification; require explicit opt-in for plaintext; warn loudly
  on `--trust-server-certificate` and offer `--trust-ca-file`; add SOCKS5
  username/password authentication; upgrade the MSSQL TLS stack off the
  EOL rustls line and add `cargo audit` to CI.
- Enforce and verify read-only semantics on MSSQL symmetrically to
  PostgreSQL; add query timeouts and server-side statement/lock timeouts;
  handle Ctrl-C by cancelling the in-flight query and restoring the
  terminal; treat failed rollbacks as fatal to the session.
- Read `.pgpass` through a single opened descriptor (fstat, not stat),
  verify file type and ownership, wrap secrets in zeroizing containers,
  and drop credential environment variables after reading them.
- Make `MetadataSnapshot` collections read-only through accessors so the
  private index cannot desynchronize; bind `Prepared` to a snapshot
  fingerprint checked at compile time; add a compilation work budget and
  a fuzz target for `QueryCompiler::compile`; add
  `#![forbid(unsafe_code)]` to the library.
- Fix the presentation cache to compute inside the cache closure and
  invalidate on refresh; build completion candidates lazily instead of
  materializing the alias cartesian product; stream Config decoding with
  a bounded in-flight byte budget.
- Restructure `main.rs` into focused modules and unify the duplicated
  PostgreSQL/MSSQL metadata pipelines behind one trait so security
  checks cannot diverge between providers again.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: terminal output becomes injection-safe and bounded;
  connections gain TLS, authenticated proxying, verified read-only
  semantics, cancellation, and timeouts; credentials are handled without
  TOCTOU windows or lingering copies.
- `onec-metadata`: snapshot lookup integrity no longer depends on callers
  leaving public collections untouched.
- `query-compilation`: prepared queries are bound to the snapshot they
  were prepared against, and compilation work is bounded end to end.

## Impact

- CLI flags added: `--sslmode`, `--insecure-plaintext`, `--trust-ca-file`,
  `--socks5-user`; `--trust-server-certificate` now prints a warning.
- New CLI dependencies: a rustls connector for tokio-postgres and
  `zeroize` (both justified in the design document); the MSSQL driver
  dependency is upgraded to drop the EOL rustls line.
- Library breaking changes: `MetadataSnapshot` collections move behind
  accessors; `Prepared::compile` rejects snapshots that do not match its
  fingerprint.
- Behavior changes only where behavior was wrong: control characters are
  escaped in output, plaintext PostgreSQL requires explicit opt-in, and
  MSSQL sessions fail fast instead of continuing after rollback errors.
