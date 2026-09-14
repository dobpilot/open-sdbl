## Why

Executing every compiled demo-corpus query against a real PostgreSQL base
— 339 statements — found thirteen that the server refuses. The goldens
could not have shown it: the SQL was exactly what the compiler intended,
and wrong only in the types it left the server to resolve.

Five faults, all of them about types:

- a value known to be `NULL` was written as a bare `NULL`, which leaves
  PostgreSQL nothing to resolve a date addition, `date_trunc`, a grouping
  key or the anchor of the hierarchy CTE against;
- `+` over strings concatenates, but the expression still reported the
  number kind a sum has, so such a value was projected into the number
  member of a composite and compared as a number;
- a stored string and an expression already rendered as text have no
  common type on PostgreSQL, so an alternative mixing them was refused;
- the same mismatch broke a comparison and a join between a stored column
  and a column a derived source projects as text;
- a path continuing past a composite hop joined its targets from the
  statement's own source instead of the alias the composite field lives
  on, naming a column that alias has not.

## What Changes

- A value known to be `NULL` SHALL state the type it stands for wherever
  the server resolves an operator or a function by it.
- `+` over strings SHALL report the string kind.
- A string alternative mixing a stored string with text SHALL be rendered
  as text, and a comparison or join between them SHALL bring the derived
  side to the stored type.
- A composite hop SHALL join its targets from the alias it lives on.

## Capabilities

### Modified Capabilities

- `query-compilation`: types the server can resolve.

## Impact

- `src/query/core/codegen/expression.rs`; `src/query/core/codegen/select.rs`;
  `src/query/core/codegen/context.rs`; `src/query/core/dialect.rs`;
  `tests/query_compile.rs`; `tests/query_types.rs`;
  `tests/fixtures/demo/expected.jsonl`; `docs/query-language-support.md`.
