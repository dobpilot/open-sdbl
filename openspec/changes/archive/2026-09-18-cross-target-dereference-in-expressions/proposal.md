## Why

A value dereferenced through a reference of several types —
`Продажи.Регистратор.Организация` — is spread over composite members
and could be projected but not compared: `= &Организация` failed as a
compound field. Three УНФ corpus queries filter that way.

## What Changes

- In an expression, such a value SHALL stand for its first member, the
  value itself (a `CASE` over the targets); compared with a reference
  constant it SHALL compare that member with the constant widened to
  its `RTRef ‖ RRRef` payload, or with the constant as it is when its
  type is unknown.

## Capabilities

### Modified Capabilities

- `query-repl`: dereferences across targets in expressions.

## Impact

`scalar_column` and `reference_member_equality` in
`src/query/core/codegen/expression.rs`.
