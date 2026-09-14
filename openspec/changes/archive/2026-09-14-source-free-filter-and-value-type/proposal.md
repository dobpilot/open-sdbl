## Why

Two constructs real queries use and the platform accepts, checked on the
probe base: a statement without a source may carry a condition —
`ВЫБРАТЬ NULL КАК Т ГДЕ ЛОЖЬ` answers no row and `ГДЕ ИСТИНА` answers one
— and a chart of characteristic types exposes `ТипЗначения`.

## What Changes

- A statement without a source SHALL accept `ГДЕ`, rendered as a `WHERE`
  without a `FROM`, which both providers accept.
- `ТипЗначения` / `ValueType` SHALL name the `Type` column of a chart of
  characteristic types.

## Capabilities

### Modified Capabilities

- `query-compilation`: a condition without a source and the value type of
  a chart of characteristic types.

## Impact

- `src/query/core/codegen/sources.rs`, `src/query/core/resolve.rs`;
  `tests/query_compile.rs`; `tests/fixtures/demo/expected.jsonl`;
  `docs/query-language-support.md`.
