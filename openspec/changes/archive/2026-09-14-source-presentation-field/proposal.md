## Why

`Источник.Представление` names the presentation of the source's own
reference, which the platform answers for a catalog and a document alike.
The compiler read the qualifier as a field name and reported it as
unknown.

## What Changes

- `Источник.Представление` / `Presentation` SHALL present the reference of
  that source, taking the same deferred path as
  `ПРЕДСТАВЛЕНИЕССЫЛКИ(Источник.Ссылка)`.
- A source without a reference of its own SHALL be refused with a
  diagnostic naming the source.

## Capabilities

### Modified Capabilities

- `query-compilation`: the presentation of a source.

## Impact

- `src/query/core/codegen/context.rs`; `tests/query_compile.rs`;
  `tests/fixtures/demo/expected.jsonl`; `docs/query-language-support.md`.
