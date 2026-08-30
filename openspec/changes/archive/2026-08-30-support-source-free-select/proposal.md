# Change: Support source-free scalar SELECT

## Why

The console rejects valid primitive query forms such as `SELECT 4` and
`SELECT PRESENTATION(4)` because every branch currently requires a metadata
source and every ordinary projection must start with a field name.

## What Changes

- Make `ИЗ`/`FROM` optional when every projection is a source-independent
  scalar expression.
- Accept literals and bounded arithmetic/logical scalar expressions in the
  projection list.
- Continue rejecting fields, wildcard projection, joins, and source-dependent
  clauses when no source is present.
- Return every scalar result as PostgreSQL text so the existing CLI row
  transport remains type-stable.

## Impact

- Affected spec: `query-repl`.
- Affected code: core query parser/compiler and regression tests.
- CLI/database interfaces do not change.
