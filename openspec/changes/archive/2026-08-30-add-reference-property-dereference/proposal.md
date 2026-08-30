## Why

Queries can currently select only fields stored in the source table. Normal 1C
query syntax navigates through a reference with a dot, for example
`Организация.Код`, and the compiler should translate that path through the
authoritative reference declaration in `SchemaStorage`.

## What Changes

- Parse one-hop reference property paths with an optional source qualifier.
- Resolve the reference target only from the `R` declaration in
  `SchemaStorage` and the DBNames/Config metadata map.
- Generate a reusable `LEFT JOIN` from the source reference column to the
  target `_IDRRef` and support the target property in projection, predicates,
  and ordering.
- Diagnose non-reference, ambiguous, non-live, and deeper paths before SQL
  execution.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: compile one-hop reference property dereference expressions.

## Impact

- Extends the dependency-free query model and PostgreSQL generator.
- Does not infer target tables from field names or PostgreSQL catalog names.
- Does not yet support paths deeper than one reference hop.
