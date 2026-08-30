## 1. Directional slice compiler

- [x] 1.1 Add bilingual SliceFirst keyword and console completion.
- [x] 1.2 Generalize source parsing and SQL generation for first/last direction.
- [x] 1.3 Preserve period, condition, alias, JOIN, tied-period, and diagnostic
  behavior for both slice kinds.

## 2. Verification

- [x] 2.1 Add lexer and compiler regressions for empty, bounded, filtered,
  joined, invalid, and SliceLast compatibility cases.
- [x] 2.2 Execute generated SliceFirst SQL against PostgreSQL `test` in a
  read-only session.
- [x] 2.3 Update documentation, pass all quality gates, and archive the strict
  OpenSpec change.
