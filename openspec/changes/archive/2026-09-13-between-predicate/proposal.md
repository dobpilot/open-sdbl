## Why

`МЕЖДУ … И …` is one of the four predicates of the 1C query language and
the natural way to write a period filter. The lexer does not know the
keyword, so such a query fails with a `Syntax` diagnostic on the word
itself.

## What Changes

- The lexer SHALL recognize `МЕЖДУ`/`BETWEEN` as a keyword that stays a
  contextual identifier.
- The parser SHALL accept `<выражение> [НЕ] МЕЖДУ <нижняя> И <верхняя>`
  at the comparison level, like `ПОДОБНО`.
- The compiler SHALL render `BETWEEN`/`NOT BETWEEN`, which matches the
  platform: the bounds are inclusive, reversed bounds select nothing, and
  a `NULL` operand does not match.

## Capabilities

### Modified Capabilities

- `sdbl-lexer`: one new bilingual keyword.
- `query-repl`: the `МЕЖДУ` predicate.

## Impact

- `src/lexer.rs`, `src/query/core/ast.rs`, `parser.rs`,
  `codegen/expression.rs`, `codegen/select.rs`, `codegen/sources.rs`;
  CLI completion; README and `docs/query-language-support.md`.
