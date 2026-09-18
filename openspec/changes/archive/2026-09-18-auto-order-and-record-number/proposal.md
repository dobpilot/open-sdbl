## Why

Three УНФ corpus queries end their ordering with `АВТОУПОРЯДОЧИВАНИЕ`
and two number the rows of a temporary table with `АВТОНОМЕРЗАПИСИ()`;
the parser refused both words.

## What Changes

- `АВТОУПОРЯДОЧИВАНИЕ` / `AUTOORDER` SHALL be accepted after the
  `УПОРЯДОЧИТЬ ПО` keys, or in their place, and change nothing: the
  platform's automatic ordering by presentations has no SQL counterpart.
- `АВТОНОМЕРЗАПИСИ()` / `RECORDAUTONUMBER()` SHALL compile to
  `ROW_NUMBER() OVER (ORDER BY (SELECT NULL))`, a number unique within
  the statement, in any statement; an argument is a `Syntax` diagnostic.

## Capabilities

### Modified Capabilities

- `sdbl-lexer`: two keywords.
- `query-repl`: automatic ordering and record numbering.

## Impact

`src/lexer.rs`, `parser.rs`, `ast.rs`, `dialect.rs`.
