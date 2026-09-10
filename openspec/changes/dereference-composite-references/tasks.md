## 1. Metadata and planning

- [ ] 1.1 Enumerate dereference candidates: declared targets, else
  reference-kind objects with the attribute; apply the 32-candidate bound
  and the work budget.
- [ ] 1.2 Plan one guarded join per candidate through the shared
  `JoinKey`, supporting compound members and payload columns split by
  dialect helpers.

## 2. SQL generation

- [ ] 2.1 Render the `CASE` value through a `ResolvedPath` expression
  override and compute the common kind with reference widening; reject
  presentation and second hops of the result.
- [ ] 2.2 Accept runtime-typed derived and temporary-table columns as
  dereference bases.

## 3. Verification and documentation

- [ ] 3.1 Goldens on both dialects: declared multi-target field, any-reference
  field resolved by attribute scan, payload column of a temporary table,
  composite dereference in `ГДЕ`, `СГРУППИРОВАТЬ ПО`, `УПОРЯДОЧИТЬ ПО`, and
  `ПО`; reference attribute widened across targets; diagnostics for no
  candidate, too many candidates, kind mismatch, presentation, and second
  hop.
- [ ] 3.2 Update README and `docs/query-language-support.md`; run
  formatting, Clippy, workspace tests, rustdoc, and strict OpenSpec
  validation.
