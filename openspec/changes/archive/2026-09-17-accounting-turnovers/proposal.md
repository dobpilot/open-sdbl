## Why

Stage 2 of the accounting-register plan: `РегистрБухгалтерии.X.Обороты`
is the second most read accounting table in the UNF corpus (turnovers by
account and organization, `СуммаОборотДт`/`СуммаОборотКт` with an
account condition). Every later aggregating table folds the record the
same way, so this stage builds the fold.

## What Changes

- `Обороты(Начало, Конец, Периодичность, УсловиеСчета, Субконто, Условие,
  УсловиеКорСчета, КорСубконто)` of a register with correspondence SHALL
  compile from the movements: each active record of `[Начало, Конец)`
  becomes a debit row and a credit row, grouped by `Счет`, the dimensions
  in use and the calendar period; a non-balance dimension or resource is
  read from the side's own column under its side-less name; per resource
  the table exposes `<Ресурс>Оборот` (debit minus credit),
  `<Ресурс>ОборотДт` and `<Ресурс>ОборотКт`; unread dimensions are summed
  away like every register table.
- `УсловиеСчета` SHALL see `Счет` (the side's account); `Условие` SHALL
  see the side's view of the record: `Счет`, the dimensions, resources,
  `Период`, `Регистратор`, `НомерСтроки`, `Активность`.
- For a register whose chart has no extra dimensions the platform omits
  the `Субконто` arguments (measured on UNF: the condition is fifth), so
  the argument layout SHALL follow the presence of the register's
  `AccRgED` table.
- The extra-dimension list, the balanced-account arguments, the `Авто`
  periodicity and a register without correspondence SHALL stay
  `UnsupportedFeature` diagnostics naming what is missing;
  `Регистратор` and `Запись` split the rows like the accumulation table.

## Capabilities

### Modified Capabilities

- `query-repl`: the accounting `Обороты` table.

## Impact

`src/query/core/codegen/accounting.rs` (new), `virtual_tables.rs`
(shared helpers); `docs/query-language-support.md`.
