## Why

`ОстаткиИОбороты` refused a periodicity outright. Measured on the probe
base, a periodic table without a balance column answers exactly what
`Обороты` answers — one row per period with movements — and the platform's
own SQL confirms why: it selects the turnovers per period and the balance
at the interval start separately, then accumulates the running balance
while reading the ordered rows, never in SQL. So the periods themselves
cost nothing to render, while the balances need a running sum SQL Server
2008 cannot express.

## What Changes

- `ОстаткиИОбороты` SHALL accept a calendar periodicity and group the
  movements of the interval into those periods, exposing `Период`.
- A period completion method SHALL be accepted together with a
  periodicity, because both methods answer the same rows where no balance
  column is read, and refused without one.
- Reading a balance column of a periodic table SHALL be refused with a
  diagnostic naming that limit.
- `Регистратор` and `Запись` SHALL be refused as periodicities of this
  table.

## Capabilities

### Modified Capabilities

- `query-compilation`: the periodicity of `ОстаткиИОбороты`.

## Impact

- `src/query/core/codegen/virtual_tables.rs`, `select.rs`;
  `tests/query_registers.rs`; README and
  `docs/query-language-support.md`.
