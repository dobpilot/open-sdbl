## 1. Implementation

- [x] 1.1 Resolve the hierarchical control point (single-target reference
  to a catalog with `_ParentIDRRef`), refuse a second one, and drop a
  duplicate plain point.
- [x] 1.2 Render the hierarchy key, the ancestor, node, and path CTEs,
  the hierarchy total rows, the path-based ordering, and the depth-based
  level column for `ИЕРАРХИЯ` and `ТОЛЬКО ИЕРАРХИЯ`; keep `WITH
  RECURSIVE` ahead of the temporary-table list.
- [x] 1.3 Add the `Родитель`/`Parent` and `Владелец`/`Owner` aliases.

## 2. Verification and documentation

- [x] 2.1 Goldens on both dialects: hierarchy, only-hierarchy, with
  `ОБЩИЕ`, with a second plain control point, the alias queries, and
  the diagnostics.
- [x] 2.2 Run the platform's hierarchy probes through the console on the
  probe base and compare rows and levels, recording the folder-order
  difference.
- [x] 2.3 Update README and `docs/query-language-support.md`; run the
  five CI checks and strict OpenSpec validation.
