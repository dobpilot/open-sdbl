## Why

A projection alias longer than the provider's label limit — 63 bytes on
PostgreSQL, 37 Cyrillic letters in the demo Бухгалтерия corpus — is
truncated in the SQL, and a nested query or temporary table then exposed
the truncated label as the field name, so the outer statement could not
find the alias the text gave.

## What Changes

- A column of a nested query or temporary table SHALL be addressable by
  the alias the text gave it, whatever label the SQL carries; the
  emitted label stays the truncated, de-duplicated one.

## Capabilities

### Modified Capabilities

- `query-repl`: long aliases of nested sources.

## Impact

`CompiledColumn` keeps the requested name crate-internally;
`derived_field` names the field by it.
