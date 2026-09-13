## 1. Implementation

- [x] 1.1 Add the keyword, the `Between` expression node, and its parsing
  at the comparison level.
- [x] 1.2 Render the predicate, and extend the fingerprint, aggregate,
  join-scope, and source-free walks.

## 2. Verification and documentation

- [x] 2.1 Goldens on both dialects and the platform probes for numbers,
  dates, strings, expressions, negation, reversed bounds, and `NULL`.
- [x] 2.2 Update README, `docs/query-language-support.md`, and CLI
  completion; run the five CI checks and strict OpenSpec validation.
