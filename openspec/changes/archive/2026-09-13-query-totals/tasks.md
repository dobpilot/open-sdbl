## 1. Parsing

- [x] 1.1 Add the five keywords (contextual), the totals AST, and its
  parsing after ordering and indexing; refuse `ИТОГИ` with
  `ПОМЕСТИТЬ`/`ДОБАВИТЬ` and in nested queries.

## 2. Rendering

- [x] 2.1 Project hidden order columns and suppress the inner `ORDER BY`
  when totals follow; render the `__totals_rows` CTE, the totals levels,
  the detail branch, and the first-appearance ordering; merge the CTE
  into the batch `WITH` list.
- [x] 2.2 Resolve control points and totals fields against result
  columns, cast counts into string columns, diagnose unknown columns and
  kind mismatches, validate `ПЕРИОДАМИ` arguments, refuse `ИЕРАРХИЯ`.
- [x] 2.3 Add `CompileOptions::totals_level` and enable it in the
  console.

## 3. Verification and documentation

- [x] 3.1 Goldens on both dialects: overall only, one and two levels,
  `ОБЩИЕ` plus levels, no totals fields, expression field with alias,
  count into a string column, union and `ПЕРВЫЕ` inputs, no `ORDER BY`,
  parameters, level column, diagnostics (`ПОМЕСТИТЬ`, nested, unknown
  column, alias naming no column, `ИЕРАРХИЯ`).
- [x] 3.2 Run the platform's totals probes through the console on the
  probe base: overall plus one level, two levels, no fields, the `NULL`
  group, `ПЕРИОДАМИ` with and without bounds and with parameters, union
  and `ПЕРВЫЕ` inputs matched row by row including the level column;
  the alias and temporary-table probes fail on both sides. A totals
  query also ran on the PostgreSQL 18 and SQL Server 2019 demo bases.
- [x] 3.3 Update README and `docs/query-language-support.md`; run the
  five CI checks and strict OpenSpec validation.
