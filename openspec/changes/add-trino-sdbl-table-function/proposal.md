## Why

The ordinary Trino table mapping exposes resolved 1C storage fields, but Trino
SQL cannot express SDBL-specific reference presentations or virtual register
tables. The core compiler already implements these semantics and must remain
their single source of truth.

## What Changes

- Add a read-only polymorphic table function
  `TABLE(onec.system.sdbl(query => '<SDBL SELECT>'))` to the Trino 476
  connector.
- Add versioned Rust HTTP operations that prepare an SDBL SELECT, describe its
  result columns, and stream its rows.
- Compile and validate the SDBL source exclusively in Rust using the existing
  metadata snapshot, presentation plans, and virtual-table compiler.
- Preserve projection and limit pushdown around the compiled SDBL relation.
- Reject non-SELECT, parameter, invalid, and unsupported SDBL input with useful
  Trino diagnostics.

## Capabilities

### Modified Capabilities

- `trino-catalog`: expose existing SDBL-only read semantics through a standard
  Trino 476 connector table function.

## Impact

- New internal prepare/scan wire messages and endpoints in `open-sdbl-trino`.
- A new `ConnectorTableFunction` and query table handle in the Java plugin.
- No Java SDBL parser, no PostgreSQL SQL supplied by Java, and no write path.
