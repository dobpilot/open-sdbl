## Context

`AggregateKind` has `Count`, `Sum`, `Min`, `Max`; the parser maps the
keyword to the kind, `compile_aggregate` renders `<NAME>(<argument>)` and
reports a number for `COUNT`/`SUM`. On the platform (probe on 2026-09-12)
`СРЕДНЕЕ(РАЗЛИЧНЫЕ …)` is a syntax error, a string argument is refused,
an empty group yields `NULL`, and the SQL is plain `AVG(x)` with numeric
literals cast to `NUMERIC`.

## Decisions

`AggregateKind::Avg` renders `AVG`, reports `Number { precision: None,
scale: None }`, and inherits the `DISTINCT`/`*` refusals. No argument
type check is added: `СУММА` has none either, and the database rejects a
non-numeric argument itself.

Result scale follows the provider: PostgreSQL returns the full `numeric`
average; SQL Server keeps six decimals for `numeric` inputs and returns an
integer average for integer inputs, which affects only literal-only
derived tables (`ВЫБРАТЬ 1 КАК Ч`), since 1C columns are `numeric`.

## Risks / Trade-offs

- The SQL Server integer-average behaviour differs from the platform,
  which casts every literal to `NUMERIC`; documented rather than emulated.
