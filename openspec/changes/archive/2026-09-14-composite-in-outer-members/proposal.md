## Why

Running the platform probes through the compiler against the same base
found three faults in the composite `В (…)` shipped in 0.3.43, none of
which the goldens showed:

- a composite value on the outer side was spread as if it were a single
  reference, so `Т.Составное В (ВЫБРАТЬ Т2.Составное …)` answered four rows
  where the platform answers nine;
- the string member of a composite result is projected as text, while the
  value compared with it stayed `mvarchar`, so PostgreSQL refused the
  statement with «оператор не существует: mvarchar = text»;
- the same fault made a string compared with a composite subquery, and a
  reference compared with a union of them, invalid SQL.

## What Changes

- A composite value on the outer side of `В (…)` SHALL answer with its own
  members instead of being spread as one of them, and SHALL be refused
  with a diagnostic when its members do not answer the subquery's.
- The string member SHALL be compared as text on both sides.

## Capabilities

### Modified Capabilities

- `query-compilation`: a composite subquery of `В (…)`.

## Impact

- `src/query/core/codegen/expression.rs`; `tests/query_refs.rs`.
