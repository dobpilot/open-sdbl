## Why

1C lets a join source carry joins of its own before its `ПО`, so the
conditions close in reverse order:
`A ЛЕВОЕ СОЕДИНЕНИЕ B ЛЕВОЕ СОЕДИНЕНИЕ C ПО <B‑C> ПО <A‑B>`. Seven real
demo queries are written that way and stop at «expected ON or ПО after
JOIN source». Measured on the probe base, a group of left joins answers
exactly what the flat chain answers, while a group whose inner join is
inner does not: it keeps the outer rows the flat chain drops.

## What Changes

- A join source SHALL accept further joins before its own `ПО`, and the
  group SHALL compile as the equivalent flat chain, outer join first.
- A group whose outer join is `ЛЕВОЕ`, `ПРАВОЕ` or `ПОЛНОЕ` and whose
  inner join is not `ЛЕВОЕ` SHALL be refused, because the flat chain
  answers differently.

## Capabilities

### Modified Capabilities

- `query-compilation`: joins nested inside a join source.

## Impact

- `src/query/core/parser.rs`; `tests/query_compile.rs`;
  `tests/fixtures/demo/expected.jsonl`; `docs/query-language-support.md`.
