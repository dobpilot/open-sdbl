## Why

Resolving one source charged the compilation work budget in proportion to
the number of live and SchemaStorage tables of the whole information base.
On a production ERP base with thousands of tables a query joining a register
with a tabular section and presenting two dereferenced references exceeded
the 32,768-unit limit before generating any SQL.

## What Changes

- Index live tables, their `X<n>` extension variants, SchemaStorage tables,
  and extension-field names in `MetadataSnapshot` at resolution time and
  expose `live_table`, `extension_live_tables`, and `schema_table` lookups.
- Replace every linear catalog scan in the query compiler with those lookups
  and charge the work budget for the work actually done (merged columns and
  extension variants), not for the size of the base.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-compilation`: work-budget accounting independent of catalog size.

## Impact

- Public API gains three `MetadataSnapshot` lookup methods.
- No generated SQL changes.
