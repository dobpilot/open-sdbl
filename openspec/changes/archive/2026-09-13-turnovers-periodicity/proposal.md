## Why

`Обороты(Начало, Конец, Месяц)` is how a 1C report asks for turnovers by
month. The compiler refused the third argument, so every periodic
turnover query had to be rewritten by hand.

## What Changes

- `Обороты` SHALL accept a calendar periodicity from `Секунда` to `Год`
  as its third argument, written as a bare period name.
- The virtual table SHALL then expose `Период`, the beginning of the
  period a record falls into, and SHALL group by it.
- The period grouping SHALL stay even when the statement never reads
  `Период`: the periodicity is an explicit request to split by period,
  and the platform answers one row per period. Measured on the probe
  base.
- `Регистратор` and `Запись` SHALL be an `UnsupportedFeature`
  diagnostic, because the virtual table does not expose the recorder
  columns they group by.

## Capabilities

### Modified Capabilities

- `query-repl`: the periodicity of `Обороты`.

## Impact

- `src/query/core/codegen/virtual_tables.rs`;
  `docs/query-language-support.md` and README.
