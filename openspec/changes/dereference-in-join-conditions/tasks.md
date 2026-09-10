## 1. SQL generation

- [ ] 1.1 Resolve one-hop dereferences in join anchors and additional
  predicates, keeping the anchor rule and scope checks on the base scope;
  reject dereferences in `ПОЛНОЕ СОЕДИНЕНИЕ` conditions.
- [ ] 1.2 Render dereference joins as parenthesized groups next to their
  source when a condition uses one; keep the flat form otherwise.

## 2. Verification and documentation

- [ ] 2.1 Goldens on both dialects: tabular section joined to its owner
  through `К.Ссылка.Основание = Д.Ссылка`, a dereference of an earlier
  source inside a later `ПО`, a dereference used only as an extra
  predicate, and an unchanged flat statement; diagnostics for two-hop paths
  and for `ПОЛНОЕ СОЕДИНЕНИЕ`.
- [ ] 2.2 Update README and `docs/query-language-support.md`; run
  formatting, Clippy, workspace tests, rustdoc, and strict OpenSpec
  validation.
