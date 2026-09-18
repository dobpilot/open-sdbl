## Why

Naming the fields of a nested query by the text's alias made two
projections that share a name — `А.Контрагент, Б.Контрагент` without
aliases — ambiguous to the outer statement; seven queries of the demo
Бухгалтерия corpus read such a source by the de-duplicated label.

## What Changes

- When several columns of a nested query or temporary table share a
  name, each SHALL be exposed under its emitted label, which the label
  allocator keeps distinct; a unique name stays the text's alias.

## Capabilities

### Modified Capabilities

- `query-repl`: shared names of derived sources.

## Impact

`derived_fields` in `src/query/core/codegen/select.rs`.
