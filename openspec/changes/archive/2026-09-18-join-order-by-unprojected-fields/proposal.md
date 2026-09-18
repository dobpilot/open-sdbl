## Why

Two УНФ corpus queries order a joined statement by a field they do not
project; the compiler ordered joined statements by projected columns
only, which the platform does not require.

## What Changes

- A joined statement without a union, a grouping or `РАЗЛИЧНЫЕ` SHALL
  order by an unprojected field as well, by the column itself; a
  projected field keeps ordering by its position.

## Capabilities

### Modified Capabilities

- `query-repl`: ordering of joined statements.

## Impact

`compile_order_terms` in `src/query/core/codegen/select.rs`.
