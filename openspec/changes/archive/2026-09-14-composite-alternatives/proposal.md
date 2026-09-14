## Why

`ВЫБОР КОГДА … ТОГДА "строка" ИНАЧЕ <ссылка> КОНЕЦ` and
`ЕСТЬNULL(<ссылка>, ЛОЖЬ)` answer a value of several types, which the
platform stores and returns by member: its own SQL, captured on the probe
base, projects the type discriminator, the member of every type present
and the reference payload. The compiler refused such a projection as
«expression kinds differ», which stopped eleven demo queries.

## What Changes

- A projected `ВЫБОР` or `ЕСТЬNULL` whose alternatives differ in type
  SHALL be rendered as the members of a composite value: the reference
  payload where a branch carries a reference, one column per other type
  present, and the `_TYPE` discriminator, each labelled with the suffix a
  projected composite field already uses.
- Every branch SHALL write its own value into its member and the zero of
  the type into the others, as the platform does.

## Capabilities

### Modified Capabilities

- `query-compilation`: alternatives of different types.

## Impact

- `src/query/core/codegen/expression.rs`, `select.rs`, `dialect.rs`;
  `tests/query_compile.rs`; `tests/fixtures/demo/expected.jsonl`;
  README and `docs/query-language-support.md`.
