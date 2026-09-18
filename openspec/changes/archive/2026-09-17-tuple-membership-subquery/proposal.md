## Why

`(Номенклатура, Характеристика) В (ВЫБРАТЬ Т.Номенклатура, Т.Характеристика ИЗ …)`
tests a pair of fields against a two-column subquery. Configurations
write it in the conditions of virtual tables and in `ГДЕ`; the compiler
stops at the comma with "expected )". 21 UNF corpus queries.

## What Changes

- A parenthesized comma list SHALL parse as a tuple, accepted only as
  the left side of `[НЕ] В (<query>)`; anywhere else it SHALL be an
  `UnsupportedFeature` diagnostic.
- The membership test SHALL require the subquery to project as many
  columns as the tuple has items, each of a kind compatible with its
  item, and SHALL render as `EXISTS` over the subquery with one equality
  per column (`NOT EXISTS` when negated), the same on both dialects. A
  reference of several types on either side SHALL be refused.

## Capabilities

### Modified Capabilities

- `query-repl`: tuple membership tests.

## Impact

`src/query/core/ast.rs`, `parser.rs`, `codegen/expression.rs`,
`codegen/select.rs`, `codegen/sources.rs`; `docs/query-language-support.md`.
