## 1. The package

- [x] 1.1 Create `crates/open-sdbl-db` with `edition = "2024"`,
  `rust-version = "1.85"`, `license = "MIT"`, version `0.6.0`,
  `publish = false`, and the dependencies moved from the CLI manifest.
- [x] 1.2 Add the package to the workspace `members`; leave
  `default-members` naming the CLI alone.
- [x] 1.3 Write `lib.rs`: the module tree, `#![warn(missing_docs)]`, and
  the crate documentation naming what the package owns.

## 2. The seams

- [x] 2.1 `DbError` with the database variants, constructors and
  predicates; `CliError::Db` with `From<DbError>`, preserved exit codes,
  preserved `Display`, and `is_broken_pipe` looking through it.
- [x] 2.2 `Limits` with `Default` equal to today's constants; sessions
  keep the value they were opened with and apply it everywhere.
- [x] 2.3 The `MetadataProgress` trait with empty default bodies and
  `NoProgress`; the terminal implementation stays in the CLI and
  implements it.
- [x] 2.4 `acquire_metadata` and the three `metadata()` methods return the
  resolution report; the CLI prints it at the three call sites.
- [x] 2.5 The connection descriptions and `Credentials`/`EnvironmentSecret`
  move; `args.rs` and `auth/pgpass.rs` keep the command line, the
  environment and the password file.
- [x] 2.6 Library functions take `&SessionParameters`; the console
  commands `\users` … `\as` stay in the CLI and call the library.

## 3. The move

- [x] 3.1 `db/` (both providers: session, metadata, cells), `net/socks5.rs`,
  `cells.rs`, `session.rs`, `limits.rs`.
- [x] 3.2 `pipeline.rs`, `extensions.rs`, `access_cache.rs`.
- [x] 3.3 `access.rs` (store, readers, listings, derivation) and
  `restrict.rs`.
- [x] 3.4 The unit tests of the moved modules, with the shared
  `enumeration_snapshot` helper lifted into `tests/support` so both
  packages can include it.
- [x] 3.5 Document every public item in English; `#[non_exhaustive]` on the
  public enumerations.

## 4. The console

- [x] 4.1 `open-sdbl-cli` depends on the new package and holds no moved
  code; `main.rs` declares only the modules that stayed.
- [x] 4.2 The REPL, `app.rs` and `args.rs` compile against the library API
  with no change in what they print.

## 5. Checks

- [x] 5.1 `cargo fmt --all -- --check`
- [x] 5.2 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 5.3 `cargo test --workspace` — the integration tests in
  `crates/open-sdbl-cli/tests` and the root `tests/` pass unmodified
- [x] 5.4 `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
- [x] 5.5 `cargo build --release --locked` and
  `cargo check --workspace --all-targets`
- [x] 5.6 `cargo tree -p open-sdbl -e normal` shows no production
  dependency, and `git diff` touches no file under `src/`
- [x] 5.7 `openspec validate extract-database-crate --strict`, then archive
