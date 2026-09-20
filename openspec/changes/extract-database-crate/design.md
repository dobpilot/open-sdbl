# Design — extract the database layer into `open-sdbl-db`

## Context

`crates/open-sdbl-cli/src` holds ~4600 lines that have nothing to do with a
terminal: two database drivers, the metadata acquisition pipeline, the
access-rights reader, the restriction expansion, the session-parameter
cache of the Standard Subsystems Library, the extension index, the SOCKS5
transport and the result cells. A second application — the Secure MCP
Gateway — needs that layer. It is security logic; it must exist once.

This change moves the code. It does not rewrite it. Behaviour of the
console — exit codes, error texts, the output of `\users`, `\roles`,
`\role`, `\rls`, `\as`, `\restrict`, `\session`, `\d` — stays identical.

The core package `open-sdbl` is not touched: zero production dependencies,
`#![forbid(unsafe_code)]`, no I/O.

## Decisions

### 1. `DbError`, and `CliError::Db`

`crate::error::CliError` mixes two roles. The database half —
`Io`, `Data`, `Database`, `DatabaseTimeout`, `MsSql`, `Metadata`, the
constructors `database_connection`, `mssql_connection`, `mssql_query`,
`socks5_connection`, and the predicates `is_database_timeout`,
`is_mssql_connection_failure`, `requires_mssql_disconnect` — becomes
`open_sdbl_db::DbError`. The console half — `Usage`, `Lexical`,
`Terminal`, `PostgresPlaintextOptInRequired`, `exit_code`,
`standard_output`, `is_broken_pipe` — stays in `CliError`, which gains
`Db(DbError)` and `From<DbError>`.

Exit codes are preserved by mapping the wrapped error the way the flat
enum did: `DbError::Metadata` and `DbError::Data` exit 1, the rest exit 2.
`is_broken_pipe` also looks through `Db(DbError::Io(..))`, so a broken
pipe raised by a driver still ends the process with success, as today.
`Display` of `CliError::Db` delegates, so every message is byte-identical.

### 2. The resolution report is returned, not printed

`acquire_metadata` printed the report through `crate::output`. It now
returns `(MetadataSnapshot, StorageLayout, ResolutionReport)`, and
`PostgresSession::metadata` / `MsSqlSession::metadata` /
`DatabaseSession::metadata` return `(MetadataSnapshot, ResolutionReport)`.
The three CLI call sites — `app::metadata`, `app::console`,
`repl::meta` (`\refresh`) — call `print_resolution_report` themselves, at
the same moment in the sequence, so the operator sees the same lines in
the same order. `output.rs` is unchanged.

Two `eprintln!` warnings stay where they are, in the Config decoder: the
malformed extension restructure and the unreadable Config resources. They
are pre-existing diagnostics the operator relies on, and routing them
through the caller would change what the console prints. They are noted as
a follow-up, not folded into this move.

### 3. `MetadataProgress` becomes a trait

The library needs the four counters; the terminal drawing is the CLI's.
`open_sdbl_db::MetadataProgress` declares `phase`, `config_totals`,
`advance_config` and `finish`, each with an empty default body, so an
application that wants no progress writes `impl MetadataProgress for X {}`.
`open_sdbl_db::NoProgress` is that implementation, ready-made.

`finish` takes `&mut self` rather than consuming the value, because the
trait is used behind `&mut dyn`. The CLI implementation keeps its
`active` flag, so `Drop` still erases a partial line exactly as before.

`acquire_metadata` no longer constructs the progress: it takes
`&mut dyn MetadataProgress` and the session methods pass it through. The
CLI constructs its terminal progress at the same three call sites that
print the report.

### 4. Connection descriptions belong to the library

`ConnectionOptions`, `PostgresConnection`, `PostgresSslMode`,
`MsSqlConnection` and `DatabaseConnection` carry no argument-parsing
attributes today — they are plain data. They move to the library as they
are, and `args.rs` keeps the whole command line: the flags, the help text,
`PGSSLMODE`, the plaintext opt-in, and the SOCKS5 proxy string. The
library therefore knows nothing about a command line, while the CLI keeps
one set of types instead of two structurally identical ones that would
have to be kept in step by hand. `Socks5Proxy` moves with the SOCKS5
transport, and `parse_socks5_proxy` stays public so `args.rs` can call it.

### 5. The secret carrier moves, the policy does not

`Credentials` and `EnvironmentSecret` (`Missing`, `InvalidUnicode`,
`Present(Zeroizing<String>)`) move with `optional` and `required`. What
reads the environment — `EnvironmentSecret::take`, with its
`env::remove_var` — and what reads `PGPASSFILE`/`~/.pgpass`, including the
ownership and permission checks, stays in `crates/open-sdbl-cli/src/auth/pgpass.rs`.
`Credentials::take_from_environment` becomes the free function
`auth::pgpass::take_credentials_from_environment`, since an inherent
method cannot be added to a type of another crate.

`socks5_password` stays with the SOCKS5 transport in the library: it reads
no environment of its own, it validates the username and the password the
caller already holds.

`PostgresSession::connect` used to call `postgres_password`, which falls
back to the password file. It now uses the secret it was handed, and the
CLI resolves that secret first: `auth::pgpass::resolve_credentials` runs
the `PGPASSWORD` → `PGPASSFILE` → `~/.pgpass` policy for a PostgreSQL
connection and hands the library a `Credentials` whose PostgreSQL secret is
already the effective one. The same error text is raised, at the same point
in the sequence — before a socket is opened.

### 6. The library takes `SessionParameters`, not the REPL store

`ParameterStore` is REPL state — `\session` parsing, listings, literals.
Every library function that needed parameters takes
`&open_sdbl::query::SessionParameters`.

One caller could not follow that rule: `apply_access_command` *writes*
into the store (`set_if_absent`) when `\as` fills the session parameters
from the base. That function is the console command layer — it parses
`\users`, `\user`, `\roles`, `\role`, `\template`, `\rls`, `\as` and
prints the answer. It therefore stays in `crates/open-sdbl-cli/src/access.rs`
together with `AccessCommand` and `parse_access_command`, and calls the
library for everything it does: `AccessStore`, `ensure_users`,
`ensure_rights`, `list_users`, `describe_user`, `list_roles`,
`describe_role`, `describe_templates`, `rls_report`, `list_restrictions`,
`derive_restrictions`, `read_template_parameters`, `read_current_user`.
The alternative — a two-method store trait in the library — would add an
abstraction the move does not need.

`restrict.rs` has no such tie and moves whole, command parsing included.

### 7. `Limits`

`limits.rs` held four constants and one SQL statement. The constants
become the fields of `open_sdbl_db::Limits` — `connection_timeout`,
`query_timeout`, `postgres_close_timeout`, `config_decode_batch_size` —
whose `Default` yields 10 s, 120 s, 5 s and 256: exactly today's values.
A session is opened with a `Limits` value, keeps it, and applies it to
every call it makes; the CLI opens sessions with `Limits::default()`, so
nothing changes for the console. `query_timeout` takes the duration it
must apply, and `bounded_database_call` is unchanged.

`MSSQL_TRANSACTION_COUNT` is a statement, not a limit; it moves next to
the SQL Server session that sends it.

### 7a. Cell decoders

The provider decoders — `PostgresCell` for the PostgreSQL binary protocol,
`mssql_row` and `decode_mssql_cell` for TDS — become public so an
application that drives a driver itself reaches the same `Cell` the console
prints. They keep their behaviour; only their visibility changes.

### 8. No `postgres` / `mssql` features

The manifest ships no optional-driver features. `DatabaseSession`,
`DatabaseDialect`, `QueryCancellation` and `DbError` are enumerations with
one variant per provider, and the access, cache and extension modules
match on the dialect in about twenty places. Gating the drivers means a
`#[cfg]` on every one of those variants and match arms, and a matrix of
feature combinations that only CI exercises. That is a rewrite of the
control flow of the very code this change promises to move unchanged, for
a saving of two client crates. The features can be added later as a change
of their own, with the combinations under test; doing it here would mix a
refactor into a move.

## Risks

- A mechanical move can drop a `#[cfg(test)]` module or silently change a
  message. Mitigation: the integration tests in `crates/open-sdbl-cli/tests`
  and the root `tests/` are not touched and must pass unchanged, and the
  unit tests travel with their modules.
- Threading `Limits` through the sessions touches every timeout call site.
  Mitigation: the value is `Copy`, the default is the old constant, and the
  tests that assert timeout messages stay as they are.

## No new dependencies

Every dependency of `open-sdbl-db` already appears in
`crates/open-sdbl-cli/Cargo.toml` and moves there: `tokio`,
`tokio-postgres`, `tokio-postgres-rustls`, `tiberius`, `rustls`,
`rustls-native-certs`, `webpki-roots`, `futures-util`, `tokio-util` and
`zeroize`. `libc` is not among them: the only `cfg(unix)` code was the
permission check of the password file, which stays in the CLI. The CLI
keeps `rustyline`, `unicode-width`, `tokio`, `zeroize` and `libc`, and
gains a path dependency on the new package.
