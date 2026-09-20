## Why

`crates/open-sdbl-db` carries `publish = false`, inherited from the CLI
manifest it was split out of. The package exists so that applications
outside this repository — the Secure MCP Gateway first — can reuse the
database layer, and a package nobody may publish can only be depended on
by path, from a checkout.

Removing the flag alone is not enough: a package whose dependency names
only a path cannot be packaged at all, so `open-sdbl-db` must name a
version for the core library it depends on.

## What Changes

- `open-sdbl-db` SHALL be publishable: the `publish = false` flag goes, and
  its dependency on `open-sdbl` SHALL carry the workspace version beside
  the path, so `cargo package` accepts it.
- The `open-sdbl-cli` application and the fuzz package SHALL stay
  unpublishable: they are binaries of this repository, not libraries for
  others.
- No code changes.

## Capabilities

### Modified Capabilities

- `crate-architecture`: which workspace packages are publishable, and what
  a publishable package's dependencies must declare.

## Impact

- `crates/open-sdbl-db/Cargo.toml`.
- Publishing `open-sdbl-db` to a registry additionally requires `open-sdbl`
  to be published there first — `cargo package` refuses otherwise, with
  "no matching package named `open-sdbl` found". Neither library is on
  crates.io today, so this change makes the package publishable without
  publishing it, and the order of the first two publishes is fixed:
  `open-sdbl`, then `open-sdbl-db`.
