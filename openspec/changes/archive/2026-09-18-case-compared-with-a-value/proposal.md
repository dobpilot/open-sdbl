## Why

`ВЫБОР … ТОГДА <ссылка> ИНАЧЕ ЛОЖЬ КОНЕЦ = &Параметр` is refused
because the alternatives differ in kind, while the platform compares
such a value and answers false for the alternative of another type.
One accounting corpus query writes it so.

## What Changes

- A `ВЫБОР` compared with a value SHALL render an alternative of
  another kind than that value as `NULL`, so the comparison never holds
  for it; compared with `NULL` or an unvalued parameter, the
  alternatives are measured against the first typed one.

## Capabilities

### Modified Capabilities

- `query-compilation`: alternatives of different types in a comparison.

## Impact

`compile_case_toward` in `src/query/core/codegen/expression.rs`, called
from `compile_expression_operand`.
