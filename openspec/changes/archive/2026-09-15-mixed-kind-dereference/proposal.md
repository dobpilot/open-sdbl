## Why

A dereference whose targets type the field differently was refused: «field
X is Reference in one reference target and String in another». Such a field
is ordinary in a real configuration — one target keeps a comment as a
string, another as a reference to a note.

Measured on 8.3.27: the platform answers one value of several types,
spreading it over the members of a composite exactly as it spreads a
`ВЫБОР` of alternatives — every target writes the member of its own type,
the zero of the others and the tag of its own type, each member staying
`NULL` while that target's value is `NULL`.

## What Changes

- A dereference whose targets disagree on the type of the field SHALL
  answer a composite value spread over the members of the types present.
- A target whose value has no place in a composite SHALL keep the
  mismatch diagnostic.

## Capabilities

### Modified Capabilities

- `query-compilation`: a dereference across targets of different types.

## Impact

- `src/query/core/codegen/context.rs`; `src/query/core/codegen/select.rs`;
  `tests/query_dereference.rs`; `docs/query-language-support.md`.
