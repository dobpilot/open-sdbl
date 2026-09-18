## Why

Seven УНФ corpus queries project or order by a dereference of a grouping
key — `СГРУППИРОВАТЬ ПО Сотрудник` with `Сотрудник.Наименование` — or
order a grouped statement by an aggregate expression; the platform
takes both, the compiler refused them.

## What Changes

- A projection, a scalar operand or an `УПОРЯДОЧИТЬ ПО` key that
  dereferences a grouping key SHALL be accepted as a function of the
  key; the joined columns it reads SHALL join the `GROUP BY` list.
- A grouping key SHALL order the statement without being projected.
- A grouped statement SHALL order by an aggregate expression.

## Capabilities

### Modified Capabilities

- `query-repl`: grouping and ordering.

## Impact

`compile_group_keys`, `scalar_is_grouped`, `compile_order_terms` in
`src/query/core/codegen/select.rs`.
