## Why

Two names the corpus stumbles on. A source written with an alias was still
addressable by its object name, so a statement that reads the same catalog
twice — once aliased, once nested — reported an ambiguous qualifier; the
platform refuses a bare object name outright, measured on the probe base,
and an alias hides it there. And a tabular section named where a field is
expected reported «field was not found», which says nothing about the
nested result the platform would return.

## What Changes

- A source that declares an alias SHALL be addressed by that alias only;
  the object name qualifies a source written without one.
- A name that resolves to no field but names a tabular section of the
  source SHALL report the nested tabular-section result, the same
  diagnostic as `Состав.*`.

## Capabilities

### Modified Capabilities

- `query-compilation`: source qualifiers and the nested tabular-section
  diagnostic.

## Impact

- `src/query/core/codegen/context.rs`; `tests/query_compile.rs`;
  `tests/fixtures/demo/expected.jsonl`; `docs/query-language-support.md`.
