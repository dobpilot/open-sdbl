## Why

`ВЫБРАТЬ РАЗЛИЧНЫЕ Т.Наименование КАК Имя … УПОРЯДОЧИТЬ ПО Имя` generated
`SELECT DISTINCT "Т"."_description"::text AS "Имя" … ORDER BY
"Т"."_description"`, which PostgreSQL refuses: with `DISTINCT` an ordering
expression must be one of the projected columns, and the projection
carries a cast that the ordering did not. Every string column is projected
with that cast, so the fault hit a very ordinary query shape; a probe run
against the platform found it.

## What Changes

- A statement with `РАЗЛИЧНЫЕ` SHALL order by its projected columns,
  addressing them by position as a grouped or unioned statement already
  does.
- An ordering field that the projection does not carry SHALL be refused
  with a diagnostic naming the reason.

## Capabilities

### Modified Capabilities

- `query-compilation`: ordering a distinct statement.

## Impact

- `src/query/core/codegen/select.rs`; `tests/query_compile.rs`;
  `docs/query-language-support.md`.
