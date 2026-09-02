## Why

Real 1C joins commonly place status and discriminator filters next to the
cross-source key equality in `ПО`/`ON`. The compiler currently rejects every
condition member that is not itself a cross-source field equality, so valid
queries using `В (...)` and `ЗНАЧЕНИЕ(...)` cannot be compiled.

## What Changes

- Keep at least one top-level cross-source direct-field equality mandatory as
  the bounded join anchor.
- Permit additional scalar predicates combined with that anchor by top-level
  `И`/`AND`.
- Compile direct-field comparisons, IN-lists, null checks, date expressions,
  and metadata values in join conditions for PostgreSQL and MSSQL.
- Continue rejecting reference-property paths and conditions without a safe
  cross-source equality anchor.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: compile bounded additional predicates in `ПО`/`ON`.

## Impact

- Extends only query AST validation and SQL generation; no metadata or I/O
  architecture changes are required.
- The existing duplicate-safe FULL JOIN transposition retains its anti-match
  marker from the mandatory equality anchor.
