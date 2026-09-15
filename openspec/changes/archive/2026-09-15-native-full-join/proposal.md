## Why

`ПОЛНОЕ СОЕДИНЕНИЕ` is not compiled into a join at all: it is transposed
into two `LEFT JOIN` branches combined by `UNION ALL` with an anti-match
predicate. That emulation is why three refusals exist — a full join may be
the only join of a branch, may carry no aggregate, and may not be grouped —
and why its condition is restricted to direct fields.

The platform accepts all four shapes. Measured on the probe base against
8.3.27: a full join chained with a left join answers 13 rows, two full
joins in a chain answer 13 rows, `СГРУППИРОВАТЬ` over a full join answers
three groups, aggregates over one answer, and a condition that dereferences
(`Т.Клиент.Наименование = К1.Наименование`) answers. The platform itself
sends no `FULL JOIN` either — it expands one into three `UNION ALL`
branches, an inner join plus two `NOT EXISTS` anti-matches — but on
equality conditions that expansion and a native `FULL JOIN` answer the
same rows, and a native join is what PostgreSQL and SQL Server both plan
directly.

The project no longer targets early PostgreSQL releases, so the emulation
buys nothing.

## What Changes

- Compile `ПОЛНОЕ [ВНЕШНЕЕ] СОЕДИНЕНИЕ` into a native `FULL JOIN` and
  remove the `UNION ALL` transposition.
- Allow a full join to appear in a chain with other joins, under
  aggregates, and under `СГРУППИРОВАТЬ`/`ГРУППИРОВАТЬ`.
- Allow a dereference in a full join condition by resolving the
  reference join inside the side that owns it, so the added `LEFT JOIN`
  cannot land after the full join.
- Keep refusing a full join condition that is not an equality chain,
  because PostgreSQL plans a full join only on merge- or hash-joinable
  conditions and would fail at execution.
- State the minimum PostgreSQL version the generated SQL targets.

## Capabilities

### Modified Capabilities

- `query-repl`: a full join is a join like the others.
- `crate-architecture`: the generated PostgreSQL targets a stated minimum
  server version instead of remaining portable to 9.0.

## Impact

Generated SQL changes for every query with `ПОЛНОЕ СОЕДИНЕНИЕ`: goldens
are re-recorded and the tests that assert the transposition are replaced.
No public API changes. The minimum supported PostgreSQL becomes 13, which
`CLAUDE.md` and the architecture spec record.
