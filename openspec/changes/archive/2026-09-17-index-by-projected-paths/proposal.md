## Why

Five queries of the demo Бухгалтерия corpus index a temporary table by
`Псевдоним.Поле` where the projection carries that field under another
alias, or by a label longer than the provider's limit; the check knew
only the emitted labels.

## What Changes

- An index field SHALL also match a column by the alias the text gave
  it when the emitted label was truncated, and a qualified index field
  SHALL match a projection whose expression is that very field path.

## Capabilities

### Modified Capabilities

- `query-repl`: index fields by name and by projected path.

## Impact

`check_index_fields` in `src/query/core/codegen/batch.rs`; `IndexAst`
keeps the field's segments.
