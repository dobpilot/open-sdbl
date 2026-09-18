## Why

Two queries of the demo Бухгалтерия corpus read
`ДвиженияССубконто(…, Порядок, Первые)`; the compiler refused the two
arguments as a later stage.

## What Changes

- `Первые` SHALL take a number literal or a parameter bound to a number
  and keep the first N records — `LIMIT N` on PostgreSQL, `TOP (N)` on
  SQL Server — in the `Порядок` given, or in the record order (period,
  recorder, line number) without one.
- `Порядок` SHALL take record fields, ascending, singly or as a tuple,
  or a parameter bound to `NULL` (no order); without `Первые` it SHALL
  have no effect, as a source's order has none and SQL Server takes no
  `ORDER BY` in a subquery. Anything else SHALL be an
  `UnsupportedFeature` diagnostic.

## Capabilities

### Modified Capabilities

- `query-repl`: the last two arguments of `ДвиженияССубконто`.

## Impact

`records_with_ext_dimensions` in `src/query/core/codegen/accounting.rs`.
