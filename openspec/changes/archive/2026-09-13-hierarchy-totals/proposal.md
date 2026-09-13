## Why

Reports over hierarchical catalogs ask for `ИТОГИ … ПО Товар ИЕРАРХИЯ`
to get folder subtotals; `query-totals` parses the modifier and refuses
it. The same reports address the folder through `Родитель`, which the
compiler exposes only under its schema name `ParentID`.

## What Changes

- `ИЕРАРХИЯ` and `ТОЛЬКО ИЕРАРХИЯ` on a control point that references a
  hierarchical catalog SHALL add hierarchy total rows for every ancestor
  folder of the values present, placed before the rows they cover, with
  the platform's levels (a folder's level is its depth in the tree, the
  rows beneath it are deeper). `ТОЛЬКО ИЕРАРХИЯ` SHALL group the rows by
  the parent folder of the value and SHALL add hierarchy rows only for
  folders above those parents.
- Folder order SHALL follow the first appearance of any row beneath the
  folder in the ordered result, the rule the plain totals use; the
  platform instead orders sibling folders by the `УПОРЯДОЧИТЬ ПО` fields
  evaluated on the folder records, which is documented as the known
  difference.
- The ancestor chain SHALL be computed with a recursive CTE over the
  catalog's `_ParentIDRRef`; PostgreSQL emits `WITH RECURSIVE`, which the
  batch `WITH` merging SHALL preserve.
- The standard fields `ParentID` and `OwnerID` SHALL also answer to
  `Родитель`/`Parent` and `Владелец`/`Owner`.
- At most one hierarchical control point per statement; a second one and
  a control point that is not a single-target reference to a hierarchical
  catalog SHALL be diagnostics.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: hierarchy totals and the two standard field aliases.

## Impact

- `src/query/core/codegen/totals.rs`, `codegen/batch.rs`,
  `src/query/core/resolve.rs` (aliases); README and
  `docs/query-language-support.md`.
- Recursive CTEs: PostgreSQL 8.4+ and SQL Server 2005+.
