## Why

Stage 3 of the accounting-register plan: `ОстаткиИОбороты` is the most
read accounting table of the UNF corpus (20 queries, opening and closing
balances by account with an account condition) and `Остатки` the next.
Both fold the record the way `Обороты` does; what is new is the balance
and its debit and credit parts.

## What Changes

- `Остатки(Период, УсловиеСчета, [Субконто], Условие)` SHALL answer, per
  account and dimensions in use, `<Ресурс>Остаток` — debit minus credit
  over the active records before `Период` — with `<Ресурс>ОстатокДт` and
  `<Ресурс>ОстатокКт` as the positive and the negated negative part of
  the balance at the grain the statement reads, and SHALL drop
  combinations whose every balance is zero.
- `ОстаткиИОбороты(Начало, Конец, Периодичность, МетодДополненияПериодов,
  УсловиеСчета, [Субконто], Условие)` SHALL answer the opening balance
  (records before `Начало`), `Оборот`/`ОборотДт`/`ОборотКт` of `[Начало,
  Конец)` and the closing balance, with the debit and credit parts of
  both balances derived at the grain read; a calendar or record split
  SHALL refuse the balance columns, `Авто` SHALL refuse them only when a
  split field is read, as the accumulation table does.
- A derived resource — a column computed from the sum of another once
  the unread dimensions are summed away — becomes part of the aggregate
  relation contract, so the parts are never sums of finer parts.
- `РазвернутыйОстаток` columns stay unexposed until measured.

## Capabilities

### Modified Capabilities

- `query-repl`: the accounting `Остатки` and `ОстаткиИОбороты` tables.

## Impact

`src/query/core/codegen/accounting.rs`, `virtual_tables.rs` (derived
resources of the aggregate relation); `docs/query-language-support.md`.
