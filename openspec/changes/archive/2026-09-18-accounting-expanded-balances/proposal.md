## Why

Three corpus queries read `<Ресурс>РазвернутыйОстатокДт/Кт` of `Остатки`
and `<Ресурс>НачальныйРазвернутыйОстатокДт` of `ОстаткиИОбороты`, the
balances expanded by account, dimensions and extra dimensions before
they are summed; the fields did not exist.

## What Changes

- `Остатки` SHALL expose `<Ресурс>РазвернутыйОстатокДт/Кт` and
  `ОстаткиИОбороты` `<Ресурс>НачальныйРазвернутыйОстатокДт/Кт` and
  `<Ресурс>КонечныйРазвернутыйОстатокДт/Кт`: the positive and the negated
  negative part of the balance per account, dimensions and extra
  dimensions, summed by the outer aggregation instead of netted.
- A periodic `ОстаткиИОбороты` SHALL refuse the expanded columns with an
  `UnsupportedFeature` diagnostic (the running sums carry no parts).
- The period completion method of the accounting `ОстаткиИОбороты` SHALL
  be accepted without a periodicity.

## Capabilities

### Modified Capabilities

- `query-repl`: expanded balances of the accounting register.

## Impact

`ResourceColumn::part`, `expanded_parts` in
`src/query/core/codegen/accounting.rs`.
