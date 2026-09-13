## 1. Implementation

- [x] 1.1 Resolve the name from the predefined values of the owning
  object as a conditional over the `PredefinedID` column, in every place
  a field is resolved, including a dereference target.
- [x] 1.2 Keep the string column kind, so PostgreSQL casts the projection
  to `text` and SQL Server spells the literals as `N'…'`.

## 2. Verification and documentation

- [x] 2.1 Platform probes for the field in a projection, a predicate, a
  grouping key and through a reference, plus the refusal on a document;
  goldens on both dialects.
- [x] 2.2 Extend the demo metadata fixture with the predefined-value
  resources and re-record the corpus.
- [x] 2.3 Update README and `docs/query-language-support.md`; run the
  five CI checks and strict OpenSpec validation.
