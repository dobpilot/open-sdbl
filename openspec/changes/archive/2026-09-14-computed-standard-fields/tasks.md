## 1. Implementation

- [x] 1.1 Resolve the two computed fields from their stored columns in
  every place a field is resolved, including a dereference target.
- [x] 1.2 Render a predicate as a boolean value per dialect.

## 2. Verification and documentation

- [x] 2.1 Platform probes for both fields in a projection, a predicate, a
  grouping key and through a reference; goldens on both dialects;
  re-record the demo corpus.
- [x] 2.2 Update README and `docs/query-language-support.md`; run the
  five CI checks and strict OpenSpec validation.
