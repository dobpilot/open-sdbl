## Why

The syntax reference lets the condition of `ДвиженияССубконто` name the
fields without a side — `Счет`, `Субконто<k>`, `ВидСубконто<k>`, a
non-balance dimension or resource — selecting the records where either
side satisfies it; the demo Бухгалтерия corpus reads
`Счет = ЗНАЧЕНИЕ(ПланСчетов.Хозрасчетный.…)` that way.

## What Changes

- The condition of `ДвиженияССубконто` SHALL accept `Счет`,
  `Субконто<k>`, `ВидСубконто<k>` and the side-less names of non-balance
  dimensions and resources; a condition that reads any of them SHALL
  hold when it holds for the debit reading or for the credit reading.
  A dereference through such a name is joined per side.

## Capabilities

### Modified Capabilities

- `query-repl`: the two-sided condition of the records table.

## Impact

`src/query/core/codegen/virtual_tables.rs` (the condition compiler takes
a mirror field list), `accounting.rs`.
