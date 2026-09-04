## 1. Terminal output safety (do first)

- [x] 1.1 Rewrite `escape_field` as a single-pass escaper covering all
  control characters, U+2028/U+2029, and bidi override characters;
  `display_width` uses the escaped form. Tests:
  `escape_field("\x1b[2J")`, OSC/BEL payloads, bidi overrides, and the
  existing `\t`/`\r`/`\n` cases unchanged.
- [x] 1.2 Convert `print_table`, `print_query_rows`, `print_snapshot`,
  and `lex` output to `&mut impl io::Write` returning `io::Result`;
  treat `BrokenPipe` as quiet success; buffer through one locked
  `BufWriter`. Test: piping into a closed pipe exits cleanly.
- [x] 1.3 Bound output: `MAX_PRINTED_ROWS` with an omission trailer,
  `MAX_CELL_WIDTH` with char-boundary ellipsis, table width capped by
  the detected terminal width; drop the intermediate full clone of the
  result set. Tests for each limit.
- [x] 1.4 Use UTF-16-independent display width accounting (CJK counts as
  two columns) for alignment. Test with a CJK fixture.

## 2. Transport security

- [x] 2.1 Add PostgreSQL TLS (`tokio-postgres-rustls`): `--sslmode`
  defaulting to `verify-full`; `disable` requires
  `--insecure-plaintext`; honor `PGSSLMODE` when the flag is absent;
  never silently downgrade. Tests: flag parsing, refusal without
  opt-in, mode precedence.
- [x] 2.2 Print a stderr warning on every `--trust-server-certificate`
  use; add `--trust-ca-file PATH` mapped to `trust_cert_ca`. Tests for
  both flags and their postgres-provider rejection.
- [x] 2.3 Upgrade the MSSQL TLS stack off rustls 0.21 so
  RUSTSEC-2026-0098/0099/0104 and unmaintained `rustls-pemfile 1.x`
  leave `Cargo.lock`; `cargo audit` passes clean.
- [x] 2.4 Add SOCKS5 username/password authentication
  (`--socks5-user` + `SOCKS5_PASSWORD`, method `0x02`); check the SOCKS5
  reply code before the reserved byte. Tests against the fake proxy
  including an auth-required scenario.
- [x] 2.5 Add a `cargo audit` CI job.

## 3. Session reliability and read-only symmetry

- [x] 3.1 Wrap every post-handshake query in a `QUERY_TIMEOUT`; set
  `statement_timeout` (PostgreSQL) and `LOCK_TIMEOUT` (MSSQL) at session
  start. Test: a stalled fake server produces a timeout error, not a
  hang.
- [x] 3.2 Add MSSQL read-only verification symmetric to
  `verify_transaction`, invoked before each query; document the
  `db_datareader` requirement in HELP and README.
- [x] 3.3 Treat a failed MSSQL `ROLLBACK` as fatal: poison the session,
  surface the error, reconnect or exit; verify `@@TRANCOUNT` returns to
  zero after errors. Unit-test the poisoning logic.
- [x] 3.4 Handle Ctrl-C during query execution via `tokio::select!`:
  cancel the PostgreSQL query with `cancel_token()`, roll back and drop
  the MSSQL connection, restore termios/scroll region on the signal
  path, exit on the second Ctrl-C.
- [x] 3.5 Bound `PostgresSession::close` with a timeout and `abort()`;
  print an acquired snapshot even when close fails (close error becomes
  a warning).
- [x] 3.6 Detect a dead session after a connection error instead of
  looping on identical failures (check `is_closed()` / reconnect or
  exit).

## 4. Credential handling

- [x] 4.1 Read `.pgpass` through one opened descriptor: `File::open`,
  then `metadata()` on the descriptor, verify regular file, owner uid,
  and permissions, read from the same handle. Tests cover fifo and
  wrong-owner rejection.
- [x] 4.2 Wrap `.pgpass` contents and all extracted passwords in
  `zeroize::Zeroizing`; remove `PGPASSWORD`/`MSSQL_PASSWORD`/
  `SOCKS5_PASSWORD` from the process environment after reading.
- [x] 4.3 Add a test asserting the CLI rejects any `--password`-style
  flag so credentials never land in argv.

## 5. Library invariants

- [ ] 5.1 Make `MetadataSnapshot` collections private with slice
  accessors; migrate all callers and tests. Test: the compiler can no
  longer be driven into out-of-bounds lookups by snapshot mutation.
- [ ] 5.2 Add a snapshot fingerprint computed during resolution; store
  it in `Prepared` and fail `Prepared::compile` with a new
  `SnapshotMismatch` diagnostic kind when it differs. Tests: same
  snapshot passes, re-resolved differing snapshot fails.
- [ ] 5.3 Add a per-compilation work budget charged in branch
  compilation, projection rendering, and dereference resolution;
  switch the tabular-section field path to the indexed catalog. Test: a
  pathological UNION × tabular-section query fails fast with a typed
  diagnostic.
- [ ] 5.4 Tighten internal invariants: `ResolvedPath::from_source` takes
  the `(index, &field)` pair; `SchemaStorage::table` and schema-table
  keys use `names_equal`; document duplicate-GUID collapsing in
  `queryable_field_catalog`.
- [ ] 5.5 Add `#![forbid(unsafe_code)]` to the library crate root.
- [ ] 5.6 Add a fuzz target compiling arbitrary source against a fixed
  synthetic snapshot.

## 6. REPL performance and correctness

- [ ] 6.1 Fix the presentation cache to compute inside the cache lookup
  (or replace moka with a session-local map cleared on `\refresh`);
  drop the generation counter from the key; rewrite the cache test to
  exercise the production path.
- [ ] 6.2 Build completion candidates lazily from the typed prefix
  instead of materializing the alias cartesian product; store
  precomputed lowercase keys and filter without per-keystroke
  allocation. Test: candidate count for a reference field stays linear
  in its own aliases.
- [ ] 6.3 History write failures log and continue; unresolved deferred
  presentations render a visible marker; cap `read_until` line length
  in non-interactive mode.

## 7. CLI structure

- [ ] 7.1 Split `main.rs` into `args`, `progress`, `net/socks5`,
  `db/postgres`, `db/mssql`, `pipeline`, `auth/pgpass`, `output`, and
  `error` modules with their tests.
- [ ] 7.2 Unify the two metadata pipelines behind a `MetadataSource`
  trait whose `begin_readonly()` hosts provider verification; the
  pipeline exists once.
- [ ] 7.3 Bound Config decoding by in-flight bytes and total decoded
  size; append descriptors incrementally instead of accumulating all
  decoded resources.
- [ ] 7.4 Harden argument parsing: reject option-like values after
  value-taking flags, support `--opt=value` and `lex --help` (or adopt
  `clap`, documenting the dependency decision).

## 8. Verification

- [ ] 8.1 `cargo fmt --check`, `cargo clippy --all-targets --
  -D warnings`, `cargo test --workspace`, rustdoc with warnings denied,
  and `cargo audit` all pass.
- [ ] 8.2 `openspec validate harden-cli-security --strict` passes.
- [ ] 8.3 README and HELP updated: TLS flags, read-only requirements,
  credential guidance, and the new library invariants.
