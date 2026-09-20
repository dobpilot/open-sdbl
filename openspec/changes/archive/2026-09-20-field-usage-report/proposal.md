## Why

Masking that hides only the output hides nothing. A statement that filters
`ГДЕ ИНН = "7701234567"` learns the value from whether rows come back, and
sorting, grouping, aggregating and `ПОДОБНО` leak it the same way.

To refuse that, an application must know not only *which* attributes a
query reads but *in what role* — projected, or used to decide which rows
exist. From outside the crate that is not knowable: the AST is private,
and the token stream shows neither position, nor aliases, nor what a
dereference resolved to.

## What Changes

- Preparation SHALL collect, beside the presentation and restriction
  requests it already collects, which `(object, field)` pairs the batch
  reads and in which roles: projection, `ГДЕ`, join condition, `ИМЕЮЩИЕ`,
  grouping, ordering, the argument of an aggregate, and part of a
  computed expression.
- A tabular section SHALL be named with its section.
- A dereference SHALL be reported against the object it ended on, not the
  object that pointed at it.
- The report SHALL be reachable from `Prepared`, in the two-phase shape
  the other requests already use, and SHALL change no generated SQL.

## Capabilities

### Modified Capabilities

- `query-compilation`: the field-usage report of a prepared query.

## Impact

- `src/query/core/restrict.rs` or a module beside it (the report types),
  `src/query/core/resolve.rs` (collection), `src/query/core/codegen/`
  (the role of the clause being compiled), `src/query.rs`.
- New tests; no change to any generated SQL.
- No new production dependency.
