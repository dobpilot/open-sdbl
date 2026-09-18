## Why

`ИЗ &Таблица` reads a value table the application passes as a
parameter. The compiler has no such parameter kind and answers "expected
metadata kind after FROM", which reads as a typo in the query; the UNF
corpus writes this in 14 queries.

## What Changes

- A parameter in a source position SHALL be an `UnsupportedFeature`
  diagnostic that says a table passed as a parameter is not supported,
  positioned at the parameter.

## Capabilities

### Modified Capabilities

- `query-repl`: the diagnostic of a parameter source.

## Impact

`src/query/core/parser.rs`, `docs/query-language-support.md`.
