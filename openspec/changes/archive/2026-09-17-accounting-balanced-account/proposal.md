## Why

Eleven queries of the demo Бухгалтерия corpus read `КорСчет` of
`Обороты` or pass the `УсловиеКорСчета` argument (`КорСчет В (…)`,
`КорСчет В ИЕРАРХИИ (…)`), two read `КорСубконто1` and
`ПодразделениеКор`; the compiler refused the balanced-account arguments
as a later stage.

## What Changes

- `Обороты` SHALL expose `КорСчет`, `<Измерение>Кор` for every
  non-balance dimension, and `КорСубконто<k>`/`ВидКорСубконто<k>`: each
  branch of the fold reads them through the opposite side's columns.
  `КорСубконто` lists kinds the way `Субконто` does, checked on the
  opposite side. `УсловиеКорСчета` and `Условие` see the same names.
  Unread, the correspondence is summed away as any dimension.

## Capabilities

### Modified Capabilities

- `query-repl`: the correspondence of `Обороты`.

## Impact

`src/query/core/codegen/accounting.rs` (`Register::correspondence`,
`extra_dimensions_of`).
