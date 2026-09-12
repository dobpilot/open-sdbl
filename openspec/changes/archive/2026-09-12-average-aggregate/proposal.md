## Why

`СРЕДНЕЕ` is the one 1C aggregate the compiler still lacks; grouped
reports (`СРЕДНЕЕ(Продажи.Цена)`) fail at the parser while both providers
have `AVG`.

## What Changes

- The lexer SHALL recognize `СРЕДНЕЕ`/`AVG` as an aggregate keyword that
  remains usable as an identifier (`КАК Среднее`).
- The compiler SHALL accept `СРЕДНЕЕ(выражение)` wherever the other
  aggregates are accepted, render it as `AVG(…)`, and report a number kind.
  `РАЗЛИЧНЫЕ` and `*` stay refused as for `СУММА`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `sdbl-lexer`: one new bilingual keyword.
- `query-repl`: the aggregate requirement gains `СРЕДНЕЕ`.

## Impact

- `src/lexer.rs`, `src/query/core/ast.rs`, `parser.rs`,
  `codegen/expression.rs`; CLI completion; README and
  `docs/query-language-support.md`.
