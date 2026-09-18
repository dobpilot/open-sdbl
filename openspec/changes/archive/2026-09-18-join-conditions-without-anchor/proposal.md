## Why

Six queries of the demo Бухгалтерия corpus join with a condition that
binds the joined source by nothing but a constant, a parameter or an
inequality — `ЛЕВОЕ СОЕДИНЕНИЕ ВТ ПО (ИСТИНА)`, `ПО (Х.Ссылка = &Счет)`,
`ПО А.Период < Б.Период` — or dereference through `ВЫРАЗИТЬ` in `ПО`;
the compiler demanded an anchor equality of direct fields for every join.

## What Changes

- An inner, left or right join SHALL take any condition its operands
  allow; only a `ПОЛНОЕ СОЕДИНЕНИЕ` SHALL keep requiring a top-level
  direct-field equality between the joined source and an earlier one.
- A dereference through `ВЫРАЗИТЬ(… КАК Тип).Реквизит` in a join
  condition SHALL join its target the way a dereferenced field does.

## Capabilities

### Modified Capabilities

- `query-repl`: join conditions.

## Impact

`compile_join_condition` and `validate_direct_join_condition_fields` in
`src/query/core/codegen/select.rs`.
