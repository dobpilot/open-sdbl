## Why

A grouped statement of the demo Бухгалтерия corpus projects `-Сумма`
over `СГРУППИРОВАТЬ ПО … Сумма`: an expression of grouped fields, which
the platform accepts, and which the compiler refused as "must be grouped
or aggregated" because only the key itself or its exact expression was
recognised.

## What Changes

- A scalar projection of a grouped statement SHALL be accepted when it
  is a function of the grouping keys: a key, a field a key names, a
  constant, or an expression whose every operand is such.

## Capabilities

### Modified Capabilities

- `query-repl`: grouped projections.

## Impact

`src/query/core/codegen/select.rs`, `sources.rs`.
