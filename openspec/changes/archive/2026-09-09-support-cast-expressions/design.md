## Context

`ВЫРАЗИТЬ` has two roles in 1C: scalar coercion and static narrowing of a
composite reference so that `.Field` can be dereferenced. The compiler already
builds type-guarded joins for presentations of multi-target references
(`ensure_presentation_join` with an `RTRef = <number>` guard), so narrowing
can reuse that plan cache instead of a second join implementation.

## Decisions

### One AST node, two target families

`Expression::Cast { argument, target, path }` where `CastTarget` is
`String { length }`, `Number { precision, scale }`, `Boolean`, `Date`, or
`Reference { kind, object }`. Type names are matched as identifiers inside
the cast, so `ДАТА` and the others remain ordinary field names elsewhere.
`path` holds at most one trailing segment and is valid only for reference
targets.

### Scalar rendering

| Target | PostgreSQL | MSSQL |
|---|---|---|
| `СТРОКА(n)` | `substring(x::text from 1 for n)` (`x::text` without n) | `CONVERT(nvarchar(n), x)` (`nvarchar(max)` when n > 4000 or absent) |
| `ЧИСЛО(p,s)` | `x::numeric(p,s)` (`::numeric` without parameters) | `CONVERT(numeric(p,s), x)` (`numeric(38,10)` by default) |
| `БУЛЕВО` | `x::boolean` | `CONVERT(bit, x)` |
| `ДАТА` | `x::timestamp` | `CONVERT(datetime2, x)` |

`substring … from … for` is used instead of `left` so PostgreSQL 9.0 stays
supported. A date cast counts as a date expression, so the MSSQL year-offset
correction applies exactly once.

### Reference narrowing

The argument must resolve to a reference field. A field with a fixed target
must name that target; a multi-target field must list it; a universal field
accepts any tabular object. The value is `CASE WHEN RTRef = <number> THEN
RRRef END` when an `RTRef` member exists, otherwise the `RRRef` itself. With a
trailing `.Field` the compiler asks the existing join cache for a
type-guarded join to the target and projects the target column with its own
kind; deeper paths and use inside `ON` are unsupported, matching existing
dereference limits.

### Boolean predicates on MSSQL

`compile_predicate` wraps predicate positions: a field or cast of boolean
kind becomes `(x = 0x01)`, `ИСТИНА`/`ЛОЖЬ` become `(1 = 1)`/`(1 = 0)`, and
`AND`/`OR`/`NOT` recurse; every other expression is compiled as before.
PostgreSQL has real booleans and keeps the bare form.

## Risks / Trade-offs

- `ЧИСЛО` without parameters relies on provider defaults; 1C's own default
  is the field's declared precision, which the compiler does not track.
- Narrowing does not yet feed `ПРЕДСТАВЛЕНИЕ`.
