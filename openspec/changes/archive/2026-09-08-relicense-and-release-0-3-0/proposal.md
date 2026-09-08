## Why

The typed-output and UUID changes break the public API of `open-sdbl`
(`CompiledQuery::columns` changed shape, generated SQL no longer casts to
text). A version bump makes that visible to dependants. The project owner has
also decided to publish the workspace under the MIT license instead of
GPL-3.0-only to remove copyleft friction for embedding the library in
connectors and services.

## What Changes

- **BREAKING** Bump `open-sdbl` and `open-sdbl-cli` from 0.1.0 to 0.3.0.
- Replace the GPL-3.0-only license with MIT: `LICENSE` text, `license`
  fields of every workspace and fuzz package, README badge and section.
- Record the licensing and versioning contract in the `crate-architecture`
  specification.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `crate-architecture`: license and version alignment of the workspace
  packages.

## Impact

- Downstream users must accept the MIT terms and the 0.3.0 API.
- The repository history has a single author, so no external consent is
  required for the relicense.
