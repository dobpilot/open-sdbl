## Why

`НЕ` is parsed as a unary operator of the operand next to it, so
`ГДЕ НЕ Т.Цена = 10` compiles to `(NOT price) = 10`, which is not what the
query says and is not valid SQL over a non-boolean column. The platform,
measured on the probe base, applies `НЕ` to the whole comparison: the same
query answers every row whose price is not 10.

## What Changes

- `НЕ` SHALL bind looser than a comparison and tighter than `И`, so it
  negates a comparison, `ПОДОБНО`, `В`, `В ИЕРАРХИИ`, `ССЫЛКА` and
  `ЕСТЬ NULL` written after it, and it groups before a conjunction.
- The unary sign operators `+` and `-` SHALL keep binding to their operand.

## Capabilities

### Modified Capabilities

- `query-compilation`: the precedence of `НЕ`.

## Impact

- `src/query/core/parser.rs`; `tests/query_compile.rs`;
  `tests/fixtures/demo/expected.jsonl`; `docs/query-language-support.md`.
