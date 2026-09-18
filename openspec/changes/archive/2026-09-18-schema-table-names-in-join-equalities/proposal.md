## Why

A join equality between a fixed reference of a derived source and a
reference of several types — `ПО Р.Регистратор = Т.Ссылка` with `Т` a
temporary table or nested query — looked the reference target up in
SchemaStorage by its physical name, `_Reference347`, while SchemaStorage
names the table `Reference347`; the lookup never matched and reported a
missing database type. Thirty-two corpus queries failed on it.

## What Changes

- The lookup SHALL compare the names without the leading underscore, so
  the equality renders the type discriminator of the fixed side.

## Capabilities

### Modified Capabilities

- `query-repl`: join equalities of fixed and runtime-typed references.

## Impact

`fixed_reference_database_type` in `src/query/core/codegen/select.rs`.
