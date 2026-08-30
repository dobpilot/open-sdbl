## 1. Parser and compiler

- [x] 1.1 Make FROM optional in the AST and parser.
- [x] 1.2 Parse bounded scalar expressions in projections.
- [x] 1.3 Compile source-free branches with stable text output and diagnostics.

## 2. Verification

- [x] 2.1 Add regressions for `SELECT 4`, `SELECT PRESENTATION(4)`, and the
  reported multiline input.
- [x] 2.2 Verify execution against PostgreSQL `test`.
- [x] 2.3 Pass all repository quality gates and archive the OpenSpec change.
