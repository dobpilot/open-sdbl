## Why

`ГДЕ Т.Объект ЕСТЬ NULL` is how a 1C query tests whether a row of an
outer join is missing when the projected field is a composite attribute.
The compiler refuses it: a composite field has several physical members,
so `single_column` reports «compound field … can be projected but not
used in expressions». Measured on the platform, the test is false for
every row of a direct composite field and true exactly when the joined
row is absent.

## What Changes

- `ЕСТЬ [НЕ] NULL` over a compound field SHALL test one representative
  member: the `_TYPE` discriminator when the field has one, otherwise the
  reference value member. Both are `NULL` exactly when the row is
  missing, so the predicate answers as the platform does.
- A compound field without either member SHALL keep the current
  diagnostic.

## Capabilities

### Modified Capabilities

- `query-repl`: `ЕСТЬ NULL` accepts a compound field.

## Impact

- `src/query/core/codegen/expression.rs`; a golden test and the
  capability table.
