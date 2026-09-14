## Why

`ВЫРАЗИТЬ(<выражение> КАК Перечисление.X)` narrows a computed value, not
only a field: real queries write it around a `ВЫБОР` that answers one of
several enumeration values. Measured on the probe base, the platform
accepts the cast when the value can hold the named type and reports
«Несовместимые типы» otherwise.

## What Changes

- `ВЫРАЗИТЬ` SHALL narrow an expression whose kind is a reference to the
  named type, keeping the value; a runtime-typed payload SHALL be narrowed
  by its type prefix, answering `NULL` for another type.
- A value that is `NULL` whatever its type, an unbound parameter included,
  SHALL narrow to `NULL` of the named type.
- Any other kind SHALL be refused, as the platform refuses it.

## Capabilities

### Modified Capabilities

- `query-compilation`: `ВЫРАЗИТЬ` over an expression.

## Impact

- `src/query/core/codegen/expression.rs`; `tests/query_compile.rs`;
  `tests/fixtures/demo/expected.jsonl`; `docs/query-language-support.md`.
