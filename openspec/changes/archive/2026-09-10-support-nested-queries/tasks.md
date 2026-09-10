## 1. Syntax

- [x] 1.1 Parse nested sources in `ИЗ` and joins (alias required) and
  `[НЕ] В (<query> | <list>)` with shared depth and budget accounting.

## 2. SQL generation

- [x] 2.1 Compile nested statements and synthesize derived-source fields
  from their columns and kinds; allow ordering only with `ПЕРВЫЕ`, allow
  inline presentations, reject deferred presentations and `*` inside nested
  queries.
- [x] 2.2 Support one-hop dereference through fixed-target derived reference
  columns and reject runtime-typed ones.
- [x] 2.3 Compile `IN (SELECT …)` with the reference member rules and
  `NOT IN` for lists and subqueries.
- [x] 2.4 Diagnose correlated references; keep derived date columns from
  being offset-corrected twice on MSSQL.

## 3. Verification and documentation

- [x] 3.1 Add PostgreSQL and MSSQL goldens (grouped nested source joined to a
  catalog, nested source with union, dereference through a derived
  reference, IN subquery for each reference combination, NOT IN list, nested
  date column on MSSQL with offset) and diagnostics tests (correlation,
  multi-column IN subquery, ordering inside nested query, runtime-typed
  dereference).
- [x] 3.2 Update README and `docs/query-language-support.md`.
- [x] 3.3 Run formatting, Clippy, workspace tests, rustdoc, and strict
  OpenSpec validation.
