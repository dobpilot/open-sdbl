## 1. Syntax

- [ ] 1.1 Parse `СГРУППИРОВАТЬ ПО` keys and `ИМЕЮЩИЕ` per branch with budget
  accounting.

## 2. SQL generation

- [ ] 2.1 Implement key matching (path, alias, normalized tokens) and the
  grouped-or-aggregated validation.
- [ ] 2.2 Emit `GROUP BY` over physical members, dereference joins, and
  inline-presentation columns.
- [ ] 2.3 Compile `HAVING` and grouped projections in aggregate-allowed
  mode (aggregates inside `ВЫБОР`); reject
  aggregates elsewhere; restrict grouped ordering; reject grouping with
  `ПОЛНОЕ СОЕДИНЕНИЕ`.

## 3. Verification and documentation

- [ ] 3.1 Add PostgreSQL and MSSQL goldens (simple grouping, reference key
  with payload projection, dereferenced key, alias key, expression key,
  HAVING, ordering by aggregate alias, UNION of grouped branches) and
  diagnostics tests.
- [ ] 3.2 Update README and `docs/query-language-support.md`.
- [ ] 3.3 Run formatting, Clippy, workspace tests, rustdoc, and strict
  OpenSpec validation.
