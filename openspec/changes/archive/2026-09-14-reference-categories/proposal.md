## Why

A type description may name a whole kind instead of one object: «любой
бизнес-процесс», «любой справочник», «любая ссылка». Those identifiers are
platform constants, so they resolved to no stored object, the field kept
its empty target list and the compiler fell back to scanning every object
carrying an attribute of the dereferenced name — which passes thirty-two
candidates and refuses.

Measured on 8.3.27 by declaring an attribute of each category in a probe
configuration and reading its Config type description: ten identifiers,
one per reference kind plus «любая ссылка».

## What Changes

- A reference type naming a kind SHALL resolve to every object of that
  kind, and «любая ссылка» to every reference of the configuration.

## Capabilities

### Modified Capabilities

- `onec-metadata`: reference categories in a type description.

## Impact

- `src/metadata/resolve.rs`; `tests/query_dereference.rs`;
  `tests/fixtures/demo/expected.jsonl`; `docs/query-language-support.md`.
