## Context

The root package currently declares both `src/lib.rs` and `src/main.rs`. The
library contains dependency-free SDBL and metadata logic, while the executable
uses `std::process::Command` to acquire data through `psql`. The requested
architecture makes the database adapter an application concern and retains the
core as deterministic transformations over caller-provided data.

## Goals / Non-Goals

**Goals:**

- Give the library and CLI independent Cargo package boundaries.
- Keep `open-sdbl` free of runtime, network, process, filesystem, and
  environment dependencies.
- Use typed PostgreSQL values instead of hexadecimal/text subprocess framing.
- Preserve existing command names and tab-separated report semantics.
- Keep authentication secrets out of argv and diagnostics.

**Non-Goals:**

- Add TLS configuration in this change; the test information base uses the
  existing non-TLS PostgreSQL connection.
- Add interactive password prompting or a command-line password option.
- Move parsing or metadata resolution into the CLI.
- Change the 1C metadata mapping algorithm.

## Decisions

### Use a package-plus-workspace root

The repository root remains the `open-sdbl` library package and also declares a
workspace whose member is `crates/open-sdbl-cli`. This avoids a disruptive move
of the public library sources and tests while ensuring the root package no
longer owns a binary target. A virtual workspace with both packages moved under
`crates/` was rejected because it would create path churn without improving the
dependency boundary.

### Keep query definitions in the library

The library will expose fixed PostgreSQL metadata query text and continue to
own decoding, normalization, and resolution. Query execution and conversion of
typed `tokio_postgres::Row` values into the library's input records belong to
the CLI. This matches the boundary “generate queries and decode data” without
introducing a database client dependency into `open-sdbl`.

### Use tokio-postgres directly

`open-sdbl-cli` will use a Tokio runtime and `tokio-postgres` 0.7 with `NoTls`.
It will build a typed connection configuration, spawn and observe the
connection future, and execute all acquisition queries inside one explicit
read-only `READ COMMITTED` transaction. The previous `psql` process, TSV
catalog transport, bytea hex encoding, `--psql` option, and process-specific
errors are removed.

### Implement libpq-compatible password lookup in the CLI

`tokio-postgres` does not load libpq password files automatically. The CLI will
prefer `PGPASSWORD`, then use `PGPASSFILE`, then `$HOME/.pgpass`. It will parse
the first matching host/port/database/user record, including `*` wildcards and
escaped colons/backslashes, and ignore password files whose Unix permissions
expose group or other bits. Secrets are passed only to the connection config
and are redacted from errors.

### Preserve the executable name

The package is named `open-sdbl-cli`, but its sole binary remains
`open-sdbl`. Existing command invocations therefore remain valid after callers
build or install the CLI package explicitly.

## Risks / Trade-offs

- **[Larger CLI dependency graph]** → Dependencies are isolated to the
  application package; the core remains dependency-free.
- **[Connection task can fail independently]** → Retain its join handle, close
  the client after acquisition, and surface task/connection failures.
- **[Password-file behavior diverges from libpq]** → Cover wildcard, escaping,
  permissions, explicit-file, and default-file cases with unit tests.
- **[Workspace commands select multiple packages]** → CI continues to use
  `--all-targets`/workspace-aware commands and README documents package-specific
  release builds.

## Migration Plan

Create the CLI package, move command code and tests, then remove the root binary
target. Build and run the new binary with `cargo run -p open-sdbl-cli -- ...` or
`cargo build --release -p open-sdbl-cli`. No information-base migration is
required; the live verification remains SELECT-only.
