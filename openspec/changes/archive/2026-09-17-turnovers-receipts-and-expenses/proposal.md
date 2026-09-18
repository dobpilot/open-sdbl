## Why

`Обороты` of a balance register answers not only `<Ресурс>Оборот` but
also `<Ресурс>Приход` and `<Ресурс>Расход` — the sums of the receipts and
of the expenses of the interval. Reports read them directly
(`ЗаказыПоставщикамОбороты.КоличествоПриход`); the compiler exposes only
the turnover and refuses the other two as unknown fields.

## What Changes

- `РегистрНакопления.X.Обороты` of a balance register SHALL expose
  `<Ресурс>Приход`/`<Resource>Receipt` and `<Ресурс>Расход`/
  `<Resource>Expense` beside `<Ресурс>Оборот`, computed from the record
  kind the way `ОстаткиИОбороты` already computes them, and summed over
  the unread dimensions like every resource column. A turnover-only
  register has no record kind and keeps exposing the turnover only.

## Capabilities

### Modified Capabilities

- `query-repl`: two more columns per resource on `Обороты`.

## Impact

`src/query/core/codegen/virtual_tables.rs`, `docs/query-language-support.md`.
