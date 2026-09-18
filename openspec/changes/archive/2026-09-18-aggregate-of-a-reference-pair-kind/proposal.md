## Why

`МАКСИМУМ(Регистратор)` renders the payload of the pair but reports a
fixed reference kind with no target, so a later `ЕСТЬNULL(…, НЕОПРЕДЕЛЕНО)`
over the aggregated column tries to widen it and fails. One accounting
corpus query (last documents by debit) reads that way.

## What Changes

- An aggregate over a reference pair SHALL carry the runtime-typed
  reference kind of the payload it renders; an aggregate over a single
  member keeps that member's kind.

## Capabilities

### Modified Capabilities

- `query-compilation`: the kind of an aggregate over a reference pair.

## Impact

`compile_aggregate` and `expression_kind` in
`src/query/core/codegen/expression.rs`; `aggregated_field_kind` goes.
