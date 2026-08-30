## Context

Vendor platform help for 8.3.27 states that union branches collect data
independently, duplicate rows are removed by default, `ВСЕ` retains them,
result fields take their names from the first branch, later fields match by
position, branch projection counts must agree, and ordering applies to the
combined result.

The compiler expands one logical compound field into several text-projected
PostgreSQL columns, so compatibility must be checked at both logical and SQL
widths before execution.

## Decisions

### Represent the complete query as branches and links

The parser produces a list of existing single-source SELECT AST nodes, one
link (`UNION` or `UNION ALL`) between adjacent branches, and one final order
list. `ORDER BY` before another union branch remains unsupported because 1C
ordering applies after the complete union.

### Compile each branch in an isolated context

Each branch resolves its own object, alias, fields, reference joins, filter,
DISTINCT, and TOP. This prevents aliases and generated joins from leaking
between branches. PostgreSQL branches are parenthesized so branch-local LIMIT
is unambiguous.

### Validate projection compatibility before returning SQL

All branches must select the same number of logical fields and emit the same
number of physical text columns. Result labels come from the first branch.
Any mismatch is a positional compilation diagnostic, so invalid SQL is never
sent to PostgreSQL.

### Order a union by first-branch output positions

Final order fields are resolved against the first branch and must occur in its
projection. PostgreSQL receives stable one-based output positions. This avoids
table-qualified references outside the branch scope and handles Russian and
English aliases consistently.

## Risks / Trade-offs

- Ordering by a field absent from the first projection is rejected in the
  bounded subset.
- Type compatibility is deterministic because every emitted physical value is
  already cast to PostgreSQL text; compound representation widths must still
  agree.
