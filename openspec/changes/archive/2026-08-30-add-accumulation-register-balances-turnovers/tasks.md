## 1. Metadata roles

- [x] 1.1 Decode accumulation-register dimension, resource, and attribute
  purposes from Config collections.
- [x] 1.2 Add parser and resolved-metadata regressions for those purposes.

## 2. Virtual source compiler

- [x] 2.1 Add bilingual Balance and Turnovers keywords and completion.
- [x] 2.2 Parse bounded Balance and four-slot Turnovers arguments without
  regressing metadata objects whose names equal virtual-table keywords.
- [x] 2.3 Generate grouped balance and turnover relations with active, period,
  direction, zero-balance, condition, alias, and JOIN semantics.
- [x] 2.4 Expose dimension and suffixed resource fields and diagnose invalid
  kinds, roles, periods, periodicity, and condition fields before execution.

## 3. Verification

- [x] 3.1 Add lexer, metadata, compiler, compatibility, and invalid-shape tests.
- [x] 3.2 Compare generated current balances with `_AccumRgT86` and execute
  balances, turnovers, filters, and JOINs read-only on PostgreSQL `test`.
- [x] 3.3 Update documentation, pass all quality gates, and archive the strict
  OpenSpec change.
