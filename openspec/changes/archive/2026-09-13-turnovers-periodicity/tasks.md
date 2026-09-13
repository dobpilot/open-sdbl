## 1. Implementation

- [x] 1.1 Read the periodicity argument, group by the truncated period,
  and expose the `Период` field.
- [x] 1.2 Keep the period grouping when the statement never reads it.

## 2. Verification and documentation

- [x] 2.1 Goldens on both dialects and platform probes for day, week,
  month, quarter and year, with and without reading the period.
- [x] 2.2 Update README and `docs/query-language-support.md`; run the
  five CI checks and strict OpenSpec validation.
