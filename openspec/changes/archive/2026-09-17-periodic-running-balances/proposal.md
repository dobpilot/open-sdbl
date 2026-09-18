## Why

A split `ОстаткиИОбороты` — by a calendar period, by the recorder, or
under `Авто` by whatever the statement reads — refused its balance
columns: the balance of a period is a running sum over the periods
before it, which the platform accumulates outside SQL and which SQL
Server 2008 cannot express. Data composition reports read exactly that
(`Регистратор`, `ПериодМесяц` with `НачальныйОстаток`); 32 UNF corpus
queries of both register kinds stop there. PostgreSQL and SQL Server
2012 have window frames.

## What Changes

- On PostgreSQL and SQL Server 2012 the balances of a split
  `ОстаткиИОбороты` of either register kind SHALL be running sums: the
  active movements before `Конец` are bucketed by the grain — the
  calendar period, the recorder, the record — with the movements before
  `Начало` as one bucket that sorts first and is dropped after the
  window; the opening balance is the sum over the previous buckets, the
  closing balance the sum up to the current one, partitioned by the
  dimensions; the debit and credit parts of an accounting balance are
  derived from the running sums.
- Under `Авто` the grain SHALL be what the statement reads — the finest
  calendar level, the recorder, or the record — so one relation is
  prepared per grain and the one read is chosen when the statement is
  known; a statement reading no balance keeps the relation without
  windows.
- On SQL Server 2008 the balances of a split table SHALL stay refused,
  the diagnostic naming the server.
- The completion method `ДвиженияИГраницыПериода` SHALL keep answering
  the buckets with movements only; the boundary rows it adds on the
  platform are not produced (documented).

## Capabilities

### Modified Capabilities

- `query-repl`: running balances of a split `ОстаткиИОбороты`.

## Impact

`src/query/core/codegen/windowed.rs` (new), `virtual_tables.rs`,
`accounting.rs`, `dialect.rs`; `docs/query-language-support.md`.
