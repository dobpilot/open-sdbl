## Why

Four queries of the demo Бухгалтерия corpus order by `Регистратор` or by
an extra dimension — compound fields of several columns — which the
compiler refused as usable in expressions; the platform orders such a
value by its type and then its value.

## What Changes

- `УПОРЯДОЧИТЬ ПО` a compound field — a reference of several types, a
  composite value, the point in time — SHALL order by each of its
  columns in turn, in the field's column order, with the term's
  direction; by path and by alias alike.

## Capabilities

### Modified Capabilities

- `query-repl`: ordering by compound fields.

## Impact

`compile_order_terms` in `src/query/core/codegen/select.rs`.
