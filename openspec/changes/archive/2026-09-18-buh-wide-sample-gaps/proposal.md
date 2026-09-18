## Why

A wide sample of the accounting configuration's module queries (200
texts that name no register) shows four gaps of ordinary syntax:
a dereferenced field without `КАК` is read back as
`ДокументРеализацииДата`, a keyword serves as an alias after `КАК`
(`КАК Конец`), a temporary table is filled by a union of `ПЕРВЫЕ`
branches ordered as a whole, and a distinct statement orders by a
projected reference field.

## What Changes

- A projected path without `КАК` SHALL be labelled by its segments
  after the source alias run together, as the platform names it.
- After `КАК`, any word that does not open the next clause SHALL be
  accepted as an alias, keywords included.
- A nested query or definition whose every branch has `ПЕРВЫЕ` MAY be
  ordered; the ordering of such a union changes nothing and SHALL be
  dropped.
- A distinct statement SHALL order by a projected reference field
  through the column the projection renders for it.
- `ССЫЛКА` SHALL accept `ВЫРАЗИТЬ(<поле> КАК <цель>)` naming its own
  target as the operand (one УНФ query).

## Capabilities

### Modified Capabilities

- `query-repl`: labels of unaliased paths; nested unions with `ПЕРВЫЕ`.
- `query-compilation`: keyword aliases after `КАК`; distinct ordering by
  a reference field.

## Impact

`context.rs` (path labels), `parser.rs` (`expect_alias`),
`orchestrate.rs` (nested ordering), `select.rs` (positional ordering).
Column labels of unaliased dereferences change: `Организация.Код`
becomes `ОрганизацияКод`.
