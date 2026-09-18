## Why

`ВЫБРАТЬ Шапка.* ИЗ Документ.X КАК Шапка` — the alias wildcard of the
statement's only source — was read as a tabular section and refused;
two УНФ queries write it.

## What Changes

- `Псевдоним.*` as the only projection of a statement with one source
  SHALL project every field of that source, as `*` does; with joins or
  other projections the path keeps naming a tabular section.

## Capabilities

### Modified Capabilities

- `query-repl`: the alias wildcard.

## Impact

`is_source_wildcard` in `src/query/core/codegen/select.rs`.
