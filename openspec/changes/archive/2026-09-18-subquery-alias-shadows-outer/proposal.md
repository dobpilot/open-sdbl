## Why

A subquery in a predicate that names its own source by the same alias
as a source of the enclosing statement — `ИЗ ДокументыКорректировки КАК
Д ГДЕ … В (ВЫБРАТЬ Д.Ссылка ИЗ ДокументыКорректировки КАК Д)` — is
refused as an ambiguous qualifier; the platform lets the inner source
hide the outer one. One accounting corpus query writes it so.

## What Changes

- A qualifier that names both a source of the subquery and a source of
  the enclosing statement SHALL resolve to the subquery's own source.

## Capabilities

### Modified Capabilities

- `query-compilation`: correlated subquery qualifier scoping.

## Impact

`qualifier_scope` in `src/query/core/codegen/context.rs`.
