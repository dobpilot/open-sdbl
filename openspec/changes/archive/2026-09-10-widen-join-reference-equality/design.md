## Context

`compile_join_field_equality` renders two single-column operands as
`left = right` and handles a compound field against a fixed reference by
comparing `_RRRef` and guarding `_RTRef` with the fixed target's type
number. Derived sources and temporary tables expose a runtime-typed
reference as one 20-byte column whose kind is `Reference { runtime_typed:
true }`, which the join path never distinguishes from a 16-byte fixed
column. `ГДЕ` comparisons already build payloads with
`SqlDialect::reference_payload` and `binary_u32`.

## Decisions

- Classify each operand as fixed (one column, `runtime_typed == false`),
  payload (one column, `runtime_typed == true`), or compound (`_RTRef` and
  `_RRRef` members). Same class on both sides keeps today's SQL, including
  the existing compound/fixed form.
- Fixed against payload: `reference_payload(binary_u32(type), id) =
  payload`; the type number comes from `fixed_reference_database_type`, so
  a fixed field without a unique target still reports its diagnostic.
- Compound against payload: `reference_payload(_RTRef, _RRRef) = payload`.
- Payload against payload: plain `=`.
- The marker of the joined side is the (possibly widened) expression, so
  `ПОЛНОЕ СОЕДИНЕНИЕ` transposition keeps working.
- Non-reference operands are untouched; a reference against a scalar keeps
  the existing diagnostic.

## Risks

Widening builds a concatenation on the fixed side, which defeats an index
on `_IDRRef` for that join. The alternative, splitting the payload column
with `SUBSTRING`, is no better for the optimizer and is less portable, so
the payload form is kept for consistency with `ГДЕ`.
