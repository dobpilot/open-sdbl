## Why

A union whose branches project values of different types was refused: the
branches disagreed either on the kind of a column or on how many columns
one value occupies.

Measured on 8.3.27: such a union is one composite value. Every branch
writes the member of its own type, the zero of every other member and the
tag of its own type, and each member stays `NULL` while the branch value
is `NULL` — the same layout a `ВЫБОР` of alternatives already produces
here.

## What Changes

- A union whose branches carry a value with different shapes SHALL spread
  that value over the members of the types present in every branch.
- A branch that already projects the members SHALL keep them, and the
  others SHALL follow its layout.
- Columns that are not the members of a composite value SHALL keep their
  diagnostic.

## Capabilities

### Modified Capabilities

- `query-compilation`: union branches of different types.

## Impact

- `src/query/core/codegen/orchestrate.rs`; `src/query/core/codegen/select.rs`;
  `src/query/core/codegen/context.rs`; `tests/query_refs.rs`;
  `tests/fixtures/demo/expected.jsonl`; `docs/query-language-support.md`.
