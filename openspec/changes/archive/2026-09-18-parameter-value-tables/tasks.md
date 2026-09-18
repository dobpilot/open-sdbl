## 1. Implementation

- [x] 1.1 `ParameterValue::Table`; the parser accepts a parameter source.
- [x] 1.2 The source scope inlines the rows as a statement CTE; kinds
  from values; references of several objects as payload.
- [x] 1.3 Console literal `ТАБЛИЦА(…)(…)`; corpus tag `T` and the
  binding tool.

## 2. Verification

- [x] 2.1 Tests for the CTE, the empty table, widening, diagnostics, SQL
  Server, the temporary-table carry and the preparation pass; the SQL
  executed on the live base; corpora rerecorded; README and the language
  table; the five CI checks.
