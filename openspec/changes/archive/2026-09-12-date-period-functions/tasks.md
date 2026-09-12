## 1. Lexer and parser

- [x] 1.1 Add `EndOfPeriod`, `DateAdd`, `DateDiff` keywords (contextual
  identifiers), `PeriodKind::Second`, and the three expression nodes.
- [x] 1.2 Parse the three functions with a shared period-argument helper
  carrying the allowed set and the `Syntax`/`UnsupportedFeature` split.

## 2. SQL generation

- [x] 2.1 Render end of period, date shift with platform rounding, and
  boundary-counting date difference on PostgreSQL and both MSSQL levels;
  report `DateTime`/`Number` kinds; shift `РАЗНОСТЬДАТ` operands to the
  logical domain under a year offset.
- [x] 2.2 Accept the new nodes and nested parameters in virtual-table
  period arguments; extend fingerprints, join-scope and aggregate walks.

## 3. Verification and documentation

- [x] 3.1 Goldens on both dialects: every period of each function,
  rounding of fractional counts, nested functions, parameters, virtual
  table with `НАЧАЛОПЕРИОДА(&П, МЕСЯЦ)`/`КОНЕЦПЕРИОДА(&П, МЕСЯЦ)`, `GROUP
  BY` key, the `Syntax` and `UnsupportedFeature` diagnostics, non-date
  arguments.
- [x] 3.2 Run the compiled SQL on the PostgreSQL and MSSQL demo servers
  over the probe dates and compare with the platform's results: every cell
  of the `КОНЕЦПЕРИОДА` (20 dates × 9 periods), `ДОБАВИТЬКДАТЕ` (19 dates ×
  20 shifts plus the fractional-count probe), and `РАЗНОСТЬДАТ` (20 dates ×
  11 differences) probes matched on PostgreSQL 18 and SQL Server 2019
  (year offset 2000). Two pre-existing defects surfaced and were fixed on
  the way: a nested source-free `SELECT` rendered MSSQL date literals in
  the logical domain although the outer statement corrects them, and a
  derived date column was typed `timestamp`, which the kind mapping reads
  as SQL Server `rowversion`.
- [x] 3.3 Update README, `docs/query-language-support.md`, CLI completion;
  run formatting, Clippy, workspace tests, rustdoc, strict OpenSpec
  validation.
