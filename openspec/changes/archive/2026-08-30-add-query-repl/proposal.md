## Why

Users can inspect the complete metadata mapping, but cannot yet explore a 1C
information base interactively. A read-only REPL should compile a useful,
explicitly bounded subset of 1C query syntax through the authoritative metadata
map and provide familiar psql-style discovery commands.

## What Changes

- Add a dependency-free core parser and PostgreSQL generator for single-source
  `ВЫБРАТЬ`/`SELECT` queries over resolved 1C metadata.
- Add `open-sdbl repl postgres` with the same connection and password options
  as `metadata postgres`.
- Execute generated statements in separate read-only `READ COMMITTED`
  transactions and render returned rows as a table.
- Add `\dt` for logical/physical table mappings, `\di` for resolved indexes,
  and `\d <metadata-name>` for object attributes and indexes.
- Add `\help` and `\q`, multiline input terminated by `;`, and recoverable
  diagnostics that keep the REPL running.

## Capabilities

### New Capabilities

- `query-repl`: Bounded 1C query compilation and interactive, read-only
  PostgreSQL exploration.

### Modified Capabilities

None.

## Impact

- Adds public query compilation and queryable-field APIs to `open-sdbl` without
  adding dependencies or I/O.
- Extends `open-sdbl-cli`; all terminal and PostgreSQL work remains isolated in
  the application crate.
- Does not claim full 1C query-language compatibility. Unsupported joins,
  grouping, unions, temporary tables, parameters, and write operations fail
  before SQL execution.
