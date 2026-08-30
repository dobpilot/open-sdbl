## 1. Generalized aggregates

- [x] 1.1 Add bilingual SUM, MIN, and MAX keywords and completion.
- [x] 1.2 Generalize COUNT AST and SQL generation to aggregate kind.
- [x] 1.3 Validate wildcard, DISTINCT, field shape, projection mixing, and FULL
  JOIN restrictions for all aggregate kinds.

## 2. Verification

- [x] 2.1 Add bilingual lexer and compiler regressions for SUM/MIN/MAX and
  COUNT DISTINCT.
- [x] 2.2 Execute all four requested aggregates against PostgreSQL `test`.
- [x] 2.3 Pass quality gates and archive the OpenSpec change.
