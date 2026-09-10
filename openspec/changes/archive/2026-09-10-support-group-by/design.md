## Context

`validate_aggregate_projection` currently enforces "all aggregates or no
aggregates" per branch. Field resolution, dereference joins, and inline
presentations all go through `CompilationContext::resolve`, so a grouped
branch can reuse them for keys. The Syntax Assistant grammar
(`platform-guides:GROUPStatement`) lists `<Разыменование поля>` keys, and
1C additionally accepts projection aliases in practice; both are supported.

## Decisions

### AST and grammar

`SelectAst { …, group: Vec<GroupKey>, having: Option<Expression> }` where
`GroupKey { token, expression: Expression }`. Clause order follows the
Syntax Assistant: `ГДЕ`, `СГРУППИРОВАТЬ ПО`, `ИМЕЮЩИЕ`. `ИМЕЮЩИЕ` without
`СГРУППИРОВАТЬ ПО` is allowed when the branch projects only aggregates (1C
allows it; SQL allows `HAVING` without `GROUP BY`).

### Key matching

A key matches a projection item when:

1. it is a field path and the item is the same resolved path (same scope,
   same field, same dereference), or
2. it is an identifier equal (case-insensitively) to the item's alias, or
3. its normalized token sequence (lexemes upper-cased, whitespace and
   comments dropped) equals the item's expression token sequence.

Every non-aggregate projection item must match a key; otherwise the
diagnostic reads `field <label> must be grouped or aggregated` at the item
token. Keys that match no projection item are still emitted (1C allows
grouping by an unprojected field). `*` is rejected in grouped branches.

### Rendering

- `GROUP BY` lists the physical columns of every key. A reference key
  contributes all its physical members (`_RTRef`, `_RRRef`, and compound
  members such as `_TYPE`, `_S`, `_N`) so the one-column payload projection
  `RTRef ‖ RRRef` is a function of grouped columns on both providers.
- A dereferenced key (`Номенклатура.Родитель`) adds its `LEFT JOIN` through
  the existing join cache and groups by the joined column.
- Inline presentations (`ПРЕДСТАВЛЕНИЕ(Ключ)` compiled to a joined text
  column) of a grouped key add their physical columns to `GROUP BY`;
  deferred reference presentations need nothing because the payload column
  is already grouped. Presentations of non-key fields are rejected by the
  key rule.
- `HAVING <predicate>` is compiled with `compile_predicate` in an
  aggregate-allowed mode. The same mode applies to projected expressions of
  a grouped branch, so `ВЫБОР КОГДА СУММА(x) > 0 ТОГДА … КОНЕЦ` is a valid
  projection; aggregates in `ГДЕ`, `ПО`, or a key are `UnsupportedFeature`
  diagnostics.
- `УПОРЯДОЧИТЬ ПО` in a grouped branch may name keys or projection aliases
  (including aggregate aliases); anything else is a diagnostic.
- `РАЗЛИЧНЫЕ` and `ПЕРВЫЕ` compose with grouping as in SQL.
- Virtual-table sources group over their outer statement unchanged.
- `ПОЛНОЕ СОЕДИНЕНИЕ` is transposed into two `LEFT JOIN` branches joined by
  `UNION ALL`; grouping over that shape would need a wrapping statement, so
  the combination is rejected until nested queries exist.

### Work budget

Each key charges one unit plus the cost of its resolution; `HAVING` charges
like `WHERE`.

## Risks / Trade-offs

- Textual key matching is stricter than 1C, which normalizes expressions
  semantically; users can always alias the projection and group by the
  alias.
- Grouping a `mchar` column on PostgreSQL groups by the extension's
  collation semantics, matching 1C.
