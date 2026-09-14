## Why

`ПРЕДСТАВЛЕНИЕ` accepted a field or a literal, so `ПРЕДСТАВЛЕНИЕ(Т.Код +
"!")` failed to parse with «expected field name» — a poor answer to valid
SDBL.

Measured on 8.3.27: the platform presents any expression. A string answers
itself, a concatenation its result, a number its digits; a reference
expression answers the presentation of the reference, which only the
application can give.

## What Changes

- `ПРЕДСТАВЛЕНИЕ` SHALL accept any expression and present a value that is
  not a reference as itself.
- A reference expression SHALL be refused with a diagnostic saying that
  only a reference field can be presented.

## Capabilities

### Modified Capabilities

- `query-compilation`: presenting an expression.

## Impact

- `src/query/core/ast.rs`; `src/query/core/parser.rs`;
  `src/query/core/codegen/context.rs`; `tests/query_compile.rs`;
  `tests/fixtures/demo/expected.jsonl`; `docs/query-language-support.md`.
