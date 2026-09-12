## 1. Implementation

- [x] 1.1 Add the `СРЕДНЕЕ`/`AVG` keyword (contextual identifier),
  `AggregateKind::Avg`, its parsing, rendering, and number kind.

## 2. Verification and documentation

- [x] 2.1 Goldens on both dialects: grouped average of a field and of an
  expression, `ИМЕЮЩИЕ СРЕДНЕЕ(…)`, `КАК Среднее` alias, `РАЗЛИЧНЫЕ`
  refusal.
- [x] 2.2 Run an average on the PostgreSQL and MSSQL demo servers: the
  literal-table probe matched the platform on PostgreSQL 18 (1.6666…,
  1.75, empty group `NULL`); on SQL Server 2019 the `numeric` averages
  matched and the integer-literal average came back as `1`, the documented
  provider behaviour.
- [x] 2.3 Update README, `docs/query-language-support.md`, CLI completion;
  run formatting, Clippy, workspace tests, rustdoc, strict OpenSpec
  validation.
