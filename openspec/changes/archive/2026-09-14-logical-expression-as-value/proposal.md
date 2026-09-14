## Why

A logical expression is a value in SDBL: the platform projects
`Т.Наименование ПОДОБНО "%а%"` as a boolean column and answers `NULL`
when an operand is `NULL` (measured on 8.3.27). The compiler refused
`ПОДОБНО` outside a predicate position, which left a demo-corpus query
uncompiled.

The other logical forms — a comparison, `И`/`ИЛИ`/`НЕ`, `ЕСТЬ NULL`,
`МЕЖДУ`, `ССЫЛКА`, `В` — were accepted as values but rendered as raw
predicates, which SQL Server does not accept in a select list: it has no
boolean values. Those statements were invalid T-SQL.

## What Changes

- `ПОДОБНО` SHALL be accepted wherever a value is expected, answering a
  boolean.
- Every logical expression used as a value SHALL be rendered in the
  dialect's boolean value form, preserving `NULL`, while a predicate
  position SHALL keep the plain predicate.

## Capabilities

### Modified Capabilities

- `query-compilation`: a logical expression used as a value.

## Impact

- `src/query/core/codegen/expression.rs`; `src/query/core/dialect.rs`;
  `tests/query_compile.rs`; `tests/fixtures/demo/expected.jsonl`;
  `docs/query-language-support.md`.
