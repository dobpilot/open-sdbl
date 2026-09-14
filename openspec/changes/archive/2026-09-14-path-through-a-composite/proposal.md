## Why

A reference path stopped at the first composite hop: `Т.Составное.Поле`
resolved, `Т.Составное.Поле.Поле` was refused with «reference path cannot
continue through X, which references more than one table». Real
configuration code walks such paths several hops deep.

Measured on 8.3.27: the platform joins every target of the composite hop
under a type guard, walks the rest of the path inside that target with
plain joins, and selects the branches with a `CASE` over the stored type.
A target in which the rest of the path does not resolve contributes no
branch at all. A composite hop met again deeper nests another `CASE`.

## What Changes

- A reference path SHALL continue past a composite hop, walking the rest
  of the path inside every target the composite may hold.
- A target in which the rest of the path does not resolve SHALL be left
  out of the result instead of failing the query.
- A hop through a field that is not a reference SHALL keep its
  diagnostic.

## Capabilities

### Modified Capabilities

- `query-compilation`: a reference path continuing past a composite hop.

## Impact

- `src/query/core/codegen/context.rs`; `tests/query_dereference.rs`;
  `tests/fixtures/demo/expected.jsonl`; `docs/query-language-support.md`.
