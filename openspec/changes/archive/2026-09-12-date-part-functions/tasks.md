## 1. Lexer and parser

- [x] 1.1 Add the ten date-part keywords as contextual identifiers and
  the `DatePart` node; parse `<function>(<expression>)`.

## 2. SQL generation

- [x] 2.1 Render the parts on PostgreSQL and MSSQL, including the
  platform week numbering and Monday-first weekday; shift to the logical
  date under a year offset; report the `Number` kind; extend fingerprints
  and the join-scope and aggregate walks.

## 3. Verification and documentation

- [x] 3.1 Goldens on both dialects: every part, `КАК Год` alias, a field
  named like a function, period names after the keywords became
  contextual, `GROUP BY ГОД(Дата)`, nested parts, non-date argument
  diagnostic.
- [x] 3.2 Run the compiled SQL on the PostgreSQL and MSSQL demo servers
  over the probe dates and compare with the platform's week and weekday
  numbers: all ten parts over twenty boundary dates, the nested
  `ГОД(КОНЕЦПЕРИОДА(ДОБАВИТЬКДАТЕ(…)))` probe, and `ДОБАВИТЬКДАТЕ(Д, ДЕНЬ,
  ДЕНЬ(Д))` matched cell by cell on PostgreSQL 18 and SQL Server 2019.
  Date arguments of unknown kind (unbound parameters during preparation)
  are now accepted by every date function, which the console's
  prepare-then-bind flow needs.
- [x] 3.3 Update README, `docs/query-language-support.md`, CLI completion;
  run formatting, Clippy, workspace tests, rustdoc, strict OpenSpec
  validation.
