## Why

`ПРЕДСТАВЛЕНИЕ` of an expression whose value is a reference was refused a
release ago, because the presentation of a reference comes from the
application. But the application already answers exactly that for a
universal reference field: the column carries the reference and the caller
resolves it in a second pass.

Measured on 8.3.27: the platform presents such an expression —
`ПРЕДСТАВЛЕНИЕ(ВЫБОР … ТОГДА Т.Клиент … КОНЕЦ)` answers «Завод», «Магазин»
— and the console resolves the deferred column to the same names.

## What Changes

- `ПРЕДСТАВЛЕНИЕ` of an expression whose value is a reference SHALL carry
  the reference as a deferred presentation column, the way a universal
  reference field does.

## Capabilities

### Modified Capabilities

- `query-compilation`: presenting an expression.

## Impact

- `src/query/core/codegen/context.rs`; `tests/query_compile.rs`;
  `tests/fixtures/demo/expected.jsonl`; `docs/query-language-support.md`.
