## Why

The database layer of the console — connecting in a verified read-only
transaction, reading and resolving 1C metadata, reading the rights of
roles, expanding the RLS restrictions of a user and decoding result
cells — lives in `crates/open-sdbl-cli/src` as `pub(crate)` items. It is
reachable only from the binary that drives it.

A second consumer now needs exactly that layer: the Secure MCP Gateway
(`dobpilot/yatagarasu`). Copying the code is not an option — it is
security logic, and two copies drift. The layer therefore moves into a
library package, and the console becomes its first consumer.

## What Changes

- A new workspace package `open-sdbl-db` SHALL own the database drivers,
  the metadata acquisition pipeline, the access-rights reading and
  restriction derivation, the restriction store, the session-parameter
  cache, the extension index, the SOCKS5 transport, the result cells and
  the limits, with a documented public API.
- `open-sdbl-cli` SHALL keep the process entry point, argument parsing,
  terminal output, progress rendering, the REPL, the `.pgpass` reader and
  its own error type, and SHALL obtain everything else from the new
  package.
- The library SHALL NOT print what it answers, parse a command line, or
  read the process environment or the file system for credentials:
  metadata acquisition SHALL return the resolution report to its caller,
  progress SHALL be reported through a trait the CLI implements,
  connection descriptions and credential carriers SHALL be plain library
  types, and the shared time and size limits SHALL be one `Limits` value
  whose `Default` is what the CLI uses today.
- The core package `open-sdbl` SHALL NOT change.

## Capabilities

### Modified Capabilities

- `crate-architecture`: a three-package workspace — the dependency-free
  core, the reusable database library, and the CLI application — with the
  boundary rules of the new library package.

## Impact

- New `crates/open-sdbl-db` (drivers, pipeline, access, restrictions,
  session-parameter cache, extensions, cells, SOCKS5, limits, session
  facade) with the unit tests of the moved modules.
- `crates/open-sdbl-cli`: `error.rs` gains a `Db` variant, `args.rs`
  builds library connection descriptions, `progress.rs` implements the
  library trait, `auth/pgpass.rs` keeps the file and environment policy,
  `app.rs` prints the resolution report, `access.rs` keeps the console
  commands.
- Root `Cargo.toml` gains the package as a workspace member.
- No new production dependency; no behavior change.
