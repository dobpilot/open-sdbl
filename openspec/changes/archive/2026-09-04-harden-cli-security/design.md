# Design

## Guiding constraints

- The core library stays dependency-free and I/O-free; every new
  dependency lands in `open-sdbl-cli` only.
- Each phase leaves the workspace green (fmt, clippy `-D warnings`,
  tests, rustdoc); security fixes land before structural refactoring.
- Existing golden SQL must not change: this campaign touches transport,
  output, and invariants — not query generation.

## 1. Terminal output safety

- Rewrite `escape_field` as a single-pass escaper: every
  `char::is_control()` character (except the intentionally readable
  `\t`, `\r`, `\n` shorthands) plus U+2028/U+2029 and the bidi controls
  U+202A..=U+202E, U+2066..=U+2069 render as `\u{...}`. `display_width`
  reuses the escaped form.
- `print_table`/`print_snapshot`/`lex` take `&mut impl io::Write` and
  return `io::Result`; `ErrorKind::BrokenPipe` terminates the process
  quietly with success. Output goes through one `BufWriter` over the
  locked stdout (fixes both the EPIPE panic and the syscall-per-line
  cost).
- New limits with constants: `MAX_PRINTED_ROWS` (with a "N rows omitted"
  trailer) and `MAX_CELL_WIDTH` (ellipsis truncation on a char
  boundary), and the table respects the detected terminal width.
- Testability: the writer parameter makes escaping, alignment, and
  truncation unit-testable; add tests including
  `escape_field("\x1b[2J")` and a CJK-width fixture.

## 2. Transport security

- PostgreSQL TLS via `tokio-postgres-rustls` (rustls is already in the
  dependency tree): `--sslmode {disable,require,verify-ca,verify-full}`
  defaulting to `verify-full`; `disable` additionally requires
  `--insecure-plaintext`. `PGSSLMODE` is honored when the flag is
  absent; an unsupported requested mode is a hard error, never a silent
  downgrade.
- MSSQL: keep encryption-required default; `--trust-server-certificate`
  prints a stderr warning on every use; add `--trust-ca-file PATH`
  mapped to `trust_cert_ca` as the safe self-signed-CA path. Upgrade
  `tiberius` (or its TLS feature) off rustls 0.21 so the three
  `rustls-webpki 0.101.7` advisories (RUSTSEC-2026-0098/0099/0104) and
  unmaintained `rustls-pemfile 1.x` leave the lock file.
- SOCKS5: offer methods `0x00` and `0x02`; credentials via
  `--socks5-user` plus `SOCKS5_PASSWORD`. Check the reply code before
  the reserved byte so real proxy errors surface. Keep domain-name
  addressing (no DNS leak) as is.
- CI: add a `cargo audit` job; deny new advisories by default.

## 3. Session reliability and read-only symmetry

- One `QUERY_TIMEOUT` wrapping every post-handshake call; server-side
  `statement_timeout` (PostgreSQL, via `Config::options`) and
  `SET LOCK_TIMEOUT` (MSSQL) at session start.
- MSSQL read-only: document and require a `db_datareader`-style login;
  add an MSSQL analogue of `verify_transaction` (checking
  `@@TRANCOUNT`/isolation from `sys.dm_exec_sessions`) invoked before
  each query, symmetric to the PostgreSQL path; treat a failed
  `ROLLBACK` as fatal — poison the session and reconnect instead of
  continuing with an open transaction.
- Cancellation: `tokio::select!` between query execution and
  `tokio::signal::ctrl_c()`; PostgreSQL uses `Client::cancel_token()`,
  MSSQL rolls back and drops the connection; a second Ctrl-C forces
  exit. Terminal restoration (termios, scroll region) also runs on the
  signal path, not only on `Drop`.
- `PostgresSession::close` gains a bounded wait with `abort()` on
  expiry, and a successfully acquired snapshot is printed even when
  closing the connection fails (the close error becomes a warning).

## 4. Credential handling

- `.pgpass`: open once, `File::metadata()` on the descriptor (removes
  the TOCTOU window), verify `is_file()`, owner uid, and the existing
  permission mask; read from the same descriptor.
- Wrap the file contents and extracted password in
  `zeroize::Zeroizing`; remove `PGPASSWORD`/`MSSQL_PASSWORD`/
  `SOCKS5_PASSWORD` from the environment after reading (documented).
- Keep the existing design decision that no `--password` flag exists;
  add a test asserting the argument parser rejects one.

## 5. Library invariants

- `MetadataSnapshot`: collections become private with slice accessors
  (`objects()`, `fields()`, `values()`, `live_tables()`, ...). This is
  the only way to keep the private positional index sound. Callers that
  mutated the snapshot (tests, CLI) migrate to constructing new
  snapshots through `resolve_metadata`.
- `Prepared<B>` stores a snapshot fingerprint (a hash over object GUIDs,
  numbers, and field numbers computed once during resolution and cached
  on the snapshot); `Prepared::compile` returns a new diagnostic kind
  `SnapshotMismatch` when the fingerprint differs.
- Compilation budget: a per-compilation work counter (charged in branch
  compilation, projection rendering, and dereference resolution) bounds
  pathological UNION × tabular-section inputs; the tabular-section path
  switches from `CustomFieldNames::Scan` to the indexed catalog.
- `ResolvedPath::from_source` takes the `(index, &field)` pair produced
  by `matching_fields` so an out-of-range index cannot be introduced;
  `SchemaStorage::table` and the catalog's schema-table keys use
  `names_equal`; `queryable_field_catalog` documents duplicate-GUID
  collapsing.
- Add `#![forbid(unsafe_code)]` to `src/lib.rs`; add a second fuzz
  target running `QueryCompiler::compile` against a fixed synthetic
  snapshot.

## 6. REPL performance and correctness

- Presentation cache: compute the plan inside the cache closure (or
  replace moka with a plain `HashMap` cleared on `\refresh` — the data
  is session-local); drop the generation counter from the key; rewrite
  the cache test to call the production path it currently bypasses.
- Completion: stop materializing the alias cartesian product; expand
  dereferenced fields lazily from the typed prefix, store precomputed
  lowercase keys, and filter by binary search. `\refresh` rebuilds only
  invalidated parts.
- History failures log and continue instead of terminating the session;
  unresolved presentations render a visible marker instead of an empty
  cell; `read_until` in non-interactive mode is capped per line.

## 7. CLI structure

- Split `main.rs` (2,111 lines) into `args`, `progress`, `net/socks5`,
  `db/postgres`, `db/mssql`, `pipeline`, `auth/pgpass`, `output`, and
  `error` modules; tests move alongside their modules.
- One metadata pipeline over a `MetadataSource` trait implemented by
  both providers; `begin_readonly()` on the trait is where both
  implementations must supply verification, making the current
  PG/MSSQL asymmetry structurally impossible.
- Config streaming: bound in-flight decoding by total bytes (semaphore
  budget), append descriptors incrementally instead of accumulating
  `decoded_resources`, and cap total decoded size with a clear error.
- Argument parsing: reject option-like values after value-taking flags,
  support `--opt=value` and `lex --help`; migrating to `clap` is
  optional and decided in-phase (new dependency must be justified
  against the manual parser's growing surface).

The implementation keeps the manual parser for this change. Its option set is
still small, the parser now has table-driven value handling and focused tests,
and adding `clap` would materially increase the CLI dependency graph without
removing provider-specific validation. Reconsider this decision when subcommands
or mutually dependent option groups grow further.

## 8. Dependency posture

New CLI dependencies and their justification:

- `tokio-postgres-rustls` — the only missing piece for PostgreSQL TLS;
  rustls itself is already compiled in via tiberius.
- `zeroize` — no-std-compatible, zero-dependency secret hygiene.
- (optional) `clap` — replaces ~140 lines of manual parsing with
  correct `--opt=value`/dedup/help behavior; decided in phase 7.
- `unicode-width` — correct column accounting for CJK output; tiny and
  dependency-free.
