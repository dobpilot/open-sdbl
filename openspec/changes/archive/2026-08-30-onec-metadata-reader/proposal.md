## Why

SDBL tooling cannot resolve logical 1C names to physical database objects from
names such as `_Reference123` alone. The project needs a deterministic,
read-only metadata reader based on the platform's `DBNames`, `Config`, and
`SchemaStorage` data so later query analysis can use the real information-base
schema.

## What Changes

- Add a dependency-free parser for the 1C brace-serialized format and raw
  DEFLATE configuration blobs.
- Resolve metadata GUIDs and database numbers from `Params.DBNames`, human
  names from bare-GUID `Config` descriptors, and physical tables, columns,
  indexes, and reference targets from `SchemaStorage`.
- Model tabular 1C metadata kinds, data-separation fields, compound physical
  columns, and PostgreSQL identifier recasing without guessing mappings from
  catalog table names.
- Add a read-only PostgreSQL CLI workflow that obtains platform metadata with
  `psql`, verifies live catalog objects, and prints the resolved mapping.
- Fail explicitly when the authoritative `DBNames` map is absent or malformed;
  do not fall back to `_ReferenceNNN`-style heuristics.

## Capabilities

### New Capabilities

- `onec-metadata`: Parse, resolve, and inspect 1C metadata and its PostgreSQL
  physical representation.

### Modified Capabilities

None.

## Impact

- Adds metadata modules and public model types to the Rust library while
  leaving the lexer independent of CLI I/O.
- Extends the `open-sdbl` executable with a `metadata postgres` command.
- Keeps Cargo free of production dependencies; PostgreSQL acquisition uses the
  installed `psql` client and a read-only session.
- Reads only 1C system tables and PostgreSQL catalogs. It does not read or
  derive platform user passwords and does not mutate the information base.
