## Why

On PostgreSQL a 1C character column has the extension type `mchar` or
`mvarchar`, whose binary wire format is undocumented, so the compiler
casts such a column to `text` when it projects it. An expression built
from such a column keeps the extension type, and nothing casts it: the
driver then fails with `error deserializing column <n>` for queries as
ordinary as `ВЫБРАТЬ ЕСТЬNULL(Л.Наименование, "нет")`,
`ВЫБОР … ТОГДА Т.Наименование … КОНЕЦ`, or `МАКСИМУМ(Наименование)`.
Found while verifying the type functions against the platform.

## What Changes

- The compiler SHALL cast a projected computed expression of kind
  `String` to `text` on PostgreSQL, in the outer statement and inside
  nested statements alike, so an operand of an extension type cannot
  reach the driver.
- The cast SHALL be skipped when the rendered expression already ends
  with a `text` cast, so ordinary `ВЫРАЗИТЬ(… КАК СТРОКА)` stays
  unchanged.

## Capabilities

### Modified Capabilities

- `query-compilation`: character results of expressions are projected as
  `text` on PostgreSQL.

## Impact

- `src/query/core/dialect.rs`, `src/query/core/codegen/select.rs`; a
  regression test over `ЕСТЬNULL`, `ВЫБОР`, and an aggregate.
