## Why

`Обороты(, , Регистратор, )` is how a report shows what each document
moved, and `Запись` shows each register record. The compiler accepted
only calendar periodicities and refused both.

## What Changes

- `Обороты` SHALL accept `Регистратор` and `Запись` beside the calendar
  periods.
- `Регистратор` SHALL group by the record's period and recorder and
  SHALL expose `Период` and `Регистратор`; `Запись` SHALL add
  `НомерСтроки` and group by it as well, so each record answers its own
  row. Measured on the platform: `НомерСтроки` is not exposed by
  `Регистратор`, only by `Запись`.
- These groupings SHALL stay even when the statement never reads the
  columns, like the calendar period.

## Capabilities

### Modified Capabilities

- `query-repl`: the recorder periodicities of `Обороты`.

## Impact

- `src/query/core/codegen/virtual_tables.rs`;
  `docs/query-language-support.md`.
