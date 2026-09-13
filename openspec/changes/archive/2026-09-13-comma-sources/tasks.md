## 1. Implementation

- [x] 1.1 Parse comma-separated source elements into `JoinKind::Cross`
  entries with no condition; keep joins attached to their element.
- [x] 1.2 Render `CROSS JOIN`, skip the condition, place separator
  predicates like the base source, enforce per-element visibility of
  join conditions, and keep the `FULL JOIN` and `*` refusals.

## 2. Verification and documentation

- [x] 2.1 Goldens on both dialects: two and three sources with a `WHERE`
  equality, a join before and after a comma, nested query, virtual
  table, temporary table and constants as comma sources, separators in
  `WHERE` and in a following `RIGHT JOIN`, the visibility diagnostic,
  `FULL JOIN` and `*` diagnostics, ambiguous unqualified field.
- [x] 2.2 Run the platform's comma-source probes (plain list, a join
  before and after the comma, a join naming another element) through the
  console on the probe base: rows matched and the visibility diagnostic
  fired where the platform reports `Поле не найдено`.
- [x] 2.3 Update README and `docs/query-language-support.md`; run the
  five CI checks and strict OpenSpec validation.
