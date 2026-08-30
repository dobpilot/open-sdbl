## Why

The `open-sdbl` package currently combines a reusable decoding library with a
binary that shells out to `psql`. Database connectivity and process I/O should
not be part of the core crate: applications need to reuse deterministic query
generation and metadata decoding without inheriting a CLI or database runtime.

## What Changes

- Convert the repository to a Cargo workspace containing the root `open-sdbl`
  library and a separate `open-sdbl-cli` application crate.
- Remove the binary target and all database/process/environment I/O from the
  root library package.
- Move the `lex` and `metadata postgres` commands into `open-sdbl-cli` while
  retaining the installed binary name `open-sdbl`.
- Replace the `psql` subprocess protocol with an asynchronous
  `tokio-postgres` connection and a read-only transaction.
- Keep PostgreSQL query text and all metadata decoding/resolution in the
  dependency-free `open-sdbl` library.
- Preserve password-file and environment authentication without accepting or
  printing a password command-line argument.

## Capabilities

### New Capabilities

- `crate-architecture`: Workspace boundaries between the reusable core library
  and the CLI application.

### Modified Capabilities

- `onec-metadata`: The live PostgreSQL adapter uses `tokio-postgres` instead of
  spawning `psql`, with equivalent read-only and authentication guarantees.

## Impact

- Adds the `tokio` and `tokio-postgres` dependency graph only to
  `open-sdbl-cli`; the root `open-sdbl` package keeps no production
  dependencies.
- Moves CLI source and integration tests under `crates/open-sdbl-cli`.
- Changes build selection for the executable to `cargo build -p open-sdbl-cli`
  while preserving the resulting executable name `open-sdbl`.
- Removes the `--psql` option because the CLI no longer starts an external
  PostgreSQL client.
