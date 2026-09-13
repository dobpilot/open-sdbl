## Context

Measured on the platform (8.3.27, probe base with a two-level catalog
`Товары`: `Сантехника` ⊃ {Кран, Вантус, Смеситель}, `Мебель` ⊃ {Стол,
Стул, `Кухня` ⊃ {Табурет}}, 2026-09-13):

- `ИТОГИ СУММА(Цена) ПО Товар ИЕРАРХИЯ` with `УПОРЯДОЧИТЬ ПО Т.Наименование`:
  `Мебель 71 (Итог по иерархии, level 0)`, `Кухня 5 (hierarchy, 1)`,
  `Табурет 5 (Итог по группировке, 2)`, `Табурет 5 (detail, 3)`, `Стол 44
  (group, 1)`, its detail (2), `Стул`, then `Сантехника 32 (hierarchy, 0)`
  and its items. A folder's level is its depth; a group total sits one
  below its folder, a detail one below its group.
- `ТОЛЬКО ИЕРАРХИЯ`: rows are grouped by the parent folder (`Кухня 5
  (group, 1)`, `Мебель 66 (group, 1)`, `Сантехника 32 (group, 0)`) and
  hierarchy rows appear only for folders above those parents (`Мебель 71
  (hierarchy, 0)`).
- A folder that is itself a key (`ТОЛЬКО ИЕРАРХИЯ`, or folders in the
  data) gets its hierarchy row first and its own group row one level
  deeper (`Мебель 71 (hierarchy, 0)` then `Мебель 66 (group, 1)`); the
  hierarchy row counts the rows keyed by the folder too.
- With `ОБЩИЕ` every level shifts by one.
- Sibling folders follow the `УПОРЯДОЧИТЬ ПО` field evaluated on the
  folder records (`Мебель` before `Сантехника` by name although the first
  ordered item belongs to `Сантехника`; by price the tie of two folders
  fell to `Сантехника`). This is not reproduced: folders are ordered by
  the first appearance of any row beneath them, consistent with plain
  totals, and the difference is documented.
- `ПО Товар, Товар ИЕРАРХИЯ` equals `ПО Товар ИЕРАРХИЯ`.

## Decisions

### Resolution

The hierarchical control point must be a fixed single-target reference
column whose target catalog's live table has `_ParentIDRRef`; otherwise
`Syntax`. A second hierarchical point is `UnsupportedFeature`. A plain
point naming the same column right before the hierarchical one is
dropped as the platform does.

### Rendering

`__totals_rows` gains `"__hk"`, the hierarchy key of a row: the control
point value (`ИЕРАРХИЯ`) or its parent from the catalog (`ТОЛЬКО
ИЕРАРХИЯ`, `LEFT JOIN` on `_IDRRef`, empty reference when absent). Two
recursive CTEs follow:

```sql
"__totals_h" AS (                       -- proper ancestors of every key
  SELECT DISTINCT r."__hk" AS "leaf", c."_parentidrref" AS "node", 1 AS "steps"
  FROM "__totals_rows" r JOIN "_reference53" c ON c."_idrref" = r."__hk"
  WHERE c."_parentidrref" <> <empty>
  UNION ALL
  SELECT h."leaf", c."_parentidrref", h."steps" + 1
  FROM "__totals_h" h JOIN "_reference53" c ON c."_idrref" = h."node"
  WHERE c."_parentidrref" <> <empty>),
"__totals_nodes" AS (                   -- every key and ancestor with rank, parent, depth
  SELECT x."node", MIN(x."rn") AS "rank", MIN(x."depth") AS "depth", MAX(x."hier") AS "hier", c."_parentidrref" AS "parent"
  FROM (SELECT r."__hk" AS "node", r."__rn" AS "rn", COALESCE(d."depth", 0) AS "depth" FROM "__totals_rows" r LEFT JOIN (SELECT "leaf", MAX("steps") AS "depth" FROM "__totals_h" GROUP BY "leaf") d ON d."leaf" = r."__hk"
        UNION ALL
        SELECT h."node", r."__rn", d."depth" - h."steps" FROM "__totals_h" h JOIN "__totals_rows" r ON r."__hk" = h."leaf" JOIN (…) d ON d."leaf" = h."leaf") x
  LEFT JOIN "_reference53" c ON c."_idrref" = x."node"
  GROUP BY x."node"),
"__totals_path" AS (                    -- materialized rank path from the roots
  SELECT n."node", <seg>(n."rank") AS "path" FROM "__totals_nodes" n
  WHERE NOT EXISTS (SELECT 1 FROM "__totals_nodes" p WHERE p."node" = n."parent")
  UNION ALL
  SELECT c."node", p."path" || '/' || <seg>(c."rank") FROM "__totals_path" p JOIN "__totals_nodes" c ON c."parent" = p."node")
```

`<seg>` zero-pads the rank to twelve digits (`LPAD` on PostgreSQL,
`RIGHT('000000000000' + …, 12)` on SQL Server, paths cast to
`varchar(4000)` there). The sort key of the hierarchical level is the
path plus a flag (`0` hierarchy row, `1` group row, `2` deeper rows), so a
folder precedes everything beneath it. Hierarchy rows exist for nodes
with `"hier" = 1` (ancestors of some key) and aggregate the rows keyed by
the node or by any leaf beneath it (`EXISTS` over `"__totals_h"`),
grouped by the shallower control points and the node; group rows and
deeper levels key on `"__hk"` and read their path and depth from the node
CTEs. The level column is `base + depth` for hierarchy rows and `base +
depth + hier` for the rows keyed by a node, so a folder's own group sits
under its hierarchy row.

The catalog lookup (`_IDRRef`, `_ParentIDRRef`) is a `UNION ALL` of the
base table and its extension tables when the catalog is extended with
data, matching the relation a source renders; separators are not
applied to the lookup.

PostgreSQL requires `WITH RECURSIVE`; `join_with_prefix` keeps the
keyword ahead of the temporary-table definitions.

### Aliases

`ParentID` gains the aliases `Родитель`/`Parent`, `OwnerID` gains
`Владелец`/`Owner` in the standard field table, so hierarchy queries can
be written the 1C way.

## Risks / Trade-offs

- Folder ordering differs from the platform when the ordering field does
  not follow first appearance; the difference is documented in the
  capabilities table.
- Very deep catalogs exceed SQL Server's default `MAXRECURSION` (100
  levels); 1C catalogs are limited to ten levels by default.
- Verified 2026-09-13: the four platform hierarchy probes match row for
  row (as multisets, levels included) on the PostgreSQL probe base; the
  recursive T-SQL ran on the MSSQL demo base over `_ДемоНоменклатура`,
  which is extended with data.
