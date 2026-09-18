## Why

Four queries of the demo Бухгалтерия corpus test a tuple with the
recorder or an extra dimension against a subquery —
`(Регистратор, НомерСтроки) В (ВЫБРАТЬ …)`, `(Субконто1, Субконто2, Субконто3) В (…)` —
which the compiler refused as references of several types.

## What Changes

- In `(…) В (ВЫБРАТЬ …)` an item that is a reference of several types
  SHALL compare its RTRef ‖ RRRef payload with the subquery column, a
  fixed reference on either side being widened to the payload as the
  single-value `В (ВЫБРАТЬ …)` does; a composite field item SHALL answer
  a composite projection with its own members, the type-reference member
  included.

## Capabilities

### Modified Capabilities

- `query-repl`: tuple membership with references of several types.

## Impact

`compile_in_tuple_query` and `composite_field_members` in
`src/query/core/codegen/expression.rs`.
