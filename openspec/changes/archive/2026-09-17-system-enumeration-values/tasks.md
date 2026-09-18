## 1. Implementation

- [x] 1.1 Parse `ЗНАЧЕНИЕ(<system enumeration>.<value>)` bilingually into
  a system-value expression carrying the stored number.
- [x] 1.2 Compile it as a numeric literal in every expression position,
  including virtual-table conditions.
- [x] 1.3 Expose `Вид`, `Забалансовый`, `Порядок` on the chart of accounts.

## 2. Verification and documentation

- [x] 2.1 Goldens on both dialects; the UNF corpus rerecorded.
- [x] 2.2 `docs/query-language-support.md`; the five CI checks and strict
  OpenSpec validation.
