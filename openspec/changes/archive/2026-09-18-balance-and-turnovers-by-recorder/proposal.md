## Why

Three queries of the demo Бухгалтерия corpus read
`РегистрНакопления.X.ОстаткиИОбороты(…, Регистратор, …)`; the
accumulation table took a calendar periodicity only, while the
accounting one already splits by the recorder and the record through
the windowed running balances.

## What Changes

- `ОстаткиИОбороты` of an accumulation register SHALL accept
  `Регистратор` and `Запись` as the periodicity: the relation splits by
  the record period and the recorder (and the line number for `Запись`),
  the balances run over those buckets where the server has window
  frames, and are refused on SQL Server 2008 as for a calendar split.

## Capabilities

### Modified Capabilities

- `query-repl`: recorder and record periodicity of the accumulation
  `ОстаткиИОбороты`.

## Impact

`compile_balance_and_turnovers_relation` in
`src/query/core/codegen/virtual_tables.rs`.
