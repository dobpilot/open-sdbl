## 1. Implementation

- [x] 1.1 Parse the genitive keyword after `В` and carry a hierarchy flag
  on both `В` shapes.
- [x] 1.2 Register one recursive CTE per predicate in the statement's
  catalog, render the `EXISTS` test, and prefix the statement with the
  definitions; degenerate to membership without a parent column.
- [x] 1.3 Refuse the predicate in a statement that defines a temporary
  table.

## 2. Verification and documentation

- [x] 2.1 Goldens on both dialects, including the negation, the
  non-hierarchical catalog, the diagnostics, and a nested query whose CTE
  is hoisted; run the platform probes.
- [x] 2.2 Update README and `docs/query-language-support.md`; run the
  five CI checks and strict OpenSpec validation.
