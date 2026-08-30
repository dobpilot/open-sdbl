## Context

Vendor platform help for 8.3.27 defines `ПОЛНОЕ [ВНЕШНЕЕ]
СОЕДИНЕНИЕ` as matched pairs plus every unmatched row from both sources,
with NULL values for the missing side. The documented English form is
`FULL [OUTER] JOIN`, and the condition marker is `ПО`/`ON`.

## Decisions

### Bound the first join grammar

One SELECT branch may contain exactly one INNER, LEFT, RIGHT, or FULL JOIN
between two resolved tabular metadata sources. Both sides must have distinct
effective aliases. Named projections, filters, and ordering may use direct
fields or one-hop reference properties from either side. The join condition is
one or more scalar cross-source direct-field equalities combined only with
`И`/`AND`. Wildcards, additional joins, and other condition shapes fail
during compilation.

### Generate non-full joins directly

INNER, LEFT, and RIGHT joins map to their quoted PostgreSQL equivalents.
Reference-property LEFT JOINs are scoped to their owning metadata source and
receive side-specific generated aliases so they cannot collide.

### Use a duplicate-safe UNION ALL transpose

Generate:

```sql
left LEFT JOIN right ON condition
UNION ALL
right LEFT JOIN left ON condition WHERE left_match_column IS NULL
```

The second anti-match predicate removes pairs already emitted by the first
branch. `UNION ALL` is required to preserve duplicate input rows and full-join
bag semantics; plain UNION would incorrectly collapse equal result rows.
Because every successful equality match has non-NULL equality operands, the
left operand of the first equality is a safe match marker even for tables
without a synthetic ID field.

### Preserve result-level operators

Wrap the transpose in an outer SELECT. Apply DISTINCT, TOP/LIMIT, and final
ordering there, not independently to each directional branch. The original
WHERE expression is compiled into both branches before the anti-match filter,
which preserves filtering after null extension.

### Keep source resolution isolated

Resolve each source through DBNames, Config, SchemaStorage, and live catalogs.
Qualified field references select one side; unqualified direct fields and
reference paths are allowed only when they resolve uniquely across both sides.
SQL identifiers remain quoted.

## Risks / Trade-offs

- The bounded equality-only ON condition intentionally excludes OR,
  non-equality, literals, functions, and reference-property paths until an
  anti-match proof exists for those forms.
- FULL JOIN can be more expensive than an inner or left join because both
  directional scans are materialized by the generated union.
