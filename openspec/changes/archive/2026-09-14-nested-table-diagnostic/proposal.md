## Why

`Справочник.Состав.(Поле, …)` and `Справочник.Состав.*` ask the platform
for a nested result set inside one column, which a single SQL statement
cannot return. Nineteen demo queries are written that way and the compiler
answered «expected field name after '.'», which says nothing about the
real reason.

## What Changes

- A nested tabular-section projection — a field path followed by `.(…)` or
  `.*` — SHALL be reported as an unsupported feature naming the nested
  result, instead of a syntax error about a missing field name.

## Capabilities

### Modified Capabilities

- `query-compilation`: the diagnostic for a nested tabular-section
  projection.

## Impact

- `src/query/core/parser.rs`; `tests/query_compile.rs`;
  `tests/fixtures/demo/expected.jsonl`; `docs/query-language-support.md`.
