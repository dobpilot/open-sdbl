## Why

`ВЫБРАТЬ РАЗЛИЧНЫЕ РАЗРЕШЕННЫЕ …` is a real spelling in the demo
configuration, and the compiler accepted only `РАЗРЕШЕННЫЕ РАЗЛИЧНЫЕ
ПЕРВЫЕ` in that one order, reporting anything else as a missing field
name.

Measured on 8.3.27: the platform accepts the three modifiers in any order
— `РАЗЛИЧНЫЕ РАЗРЕШЕННЫЕ`, `РАЗРЕШЕННЫЕ РАЗЛИЧНЫЕ`, `ПЕРВЫЕ 1
РАЗЛИЧНЫЕ`, `ПЕРВЫЕ 1 РАЗРЕШЕННЫЕ` and both three-word orders all answer
what the canonical order answers.

## What Changes

- `РАЗРЕШЕННЫЕ`, `РАЗЛИЧНЫЕ` and `ПЕРВЫЕ <n>` SHALL be accepted in any
  order after `ВЫБРАТЬ`, each at most once.

## Capabilities

### Modified Capabilities

- `query-compilation`: the order of the selection modifiers.

## Impact

- `src/query/core/parser.rs`; `tests/query_compile.rs`;
  `tests/fixtures/demo/expected.jsonl`; `docs/query-language-support.md`.
