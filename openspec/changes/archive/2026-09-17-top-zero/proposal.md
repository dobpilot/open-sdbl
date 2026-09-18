## Why

Configurations write `ВЫБРАТЬ ПЕРВЫЕ 0 …` to take the shape of a table
without its rows — the UNF corpus does it in 29 queries that build an
empty temporary table before a union. The platform accepts a zero count;
the compiler refuses it as a syntax error.

## What Changes

- `ПЕРВЫЕ 0`/`TOP 0` SHALL compile to `LIMIT 0` on PostgreSQL and
  `TOP (0)` on SQL Server, both of which answer no rows.

## Capabilities

### Modified Capabilities

- `query-repl`: a zero row count.

## Impact

`src/query/core/parser.rs`, `docs/query-language-support.md`.
