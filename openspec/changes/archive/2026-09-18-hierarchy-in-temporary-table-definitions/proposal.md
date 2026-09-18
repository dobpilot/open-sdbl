## Why

`В ИЕРАРХИИ (…)` compiles to a recursive CTE attached to the statement,
which a `ПОМЕСТИТЬ` statement could not carry: its body becomes a CTE
itself, and SQL Server takes no `WITH` inside a CTE. Two queries of the
demo Бухгалтерия corpus filter a temporary table by a hierarchy.

## What Changes

- A statement defining a temporary table SHALL keep its hierarchy CTEs
  as sibling definitions, named per table, rendered before the table's
  own CTE in the `WITH` list of every statement that reads it
  (`WITH RECURSIVE` on PostgreSQL).

## Capabilities

### Modified Capabilities

- `query-repl`: `В ИЕРАРХИИ` in temporary-table definitions.

## Impact

`TempTableEntry` carries the CTEs; `compile_statement` and
`place_statement` in `src/query/core/codegen/batch.rs`.
