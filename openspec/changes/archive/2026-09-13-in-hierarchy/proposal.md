## Why

`В ИЕРАРХИИ` is how a 1C query selects everything under a folder, and it
is one of the four predicates of the language. The compiler does not know
it, so the query fails on the word after `В`.

## What Changes

- The parser SHALL accept `<поле> [НЕ] В ИЕРАРХИИ (<список> | <запрос>)`
  wherever `В` is accepted, recognizing the genitive `ИЕРАРХИИ` by
  lexeme, as `ВОЗР` and `УБЫВ` already are.
- The compiler SHALL render the descent as one recursive CTE per
  predicate over the catalog's parent column, defined at statement level
  and referenced by an `EXISTS` predicate, so a value matches a seed or
  any of its descendants. A catalog without a parent column SHALL
  degenerate to plain membership, which is what the platform answers.
  The empty reference SHALL therefore match the whole catalog, again as
  on the platform.
- The seeds SHALL be a nested query or a list of constants; a field is an
  `UnsupportedFeature` diagnostic, because a CTE cannot read the outer
  row. The tested value SHALL be a field referencing one catalog.
- A statement that defines a temporary table SHALL refuse the predicate,
  because its body becomes a CTE and SQL Server forbids a nested `WITH`.

## Capabilities

### Modified Capabilities

- `query-repl`: the `В ИЕРАРХИИ` predicate.

## Impact

- `src/query/core/ast.rs`, `parser.rs`, `codegen/expression.rs`,
  `codegen/orchestrate.rs`, `codegen/batch.rs`, `codegen/totals.rs`
  (shared catalog lookup), `resolve.rs` (statement CTE list); README and
  `docs/query-language-support.md`.
