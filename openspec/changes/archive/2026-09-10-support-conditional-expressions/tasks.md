## 1. Syntax

- [x] 1.1 Add `ЕСТЬNULL`/`ISNULL`, `ПОДОБНО`/`LIKE`, and `СПЕЦСИМВОЛ`/`ESCAPE`
  to the keyword table (51 entries) and the exhaustive lexer test.
- [x] 1.2 Parse `ВЫБОР … КОНЕЦ`, `ЕСТЬNULL(x, y)`, and `[НЕ] ПОДОБНО …
  [СПЕЦСИМВОЛ …]` into the new `Expression` variants with depth and budget
  accounting.
- [x] 1.3 Parse aggregate arguments as expressions.

## 2. SQL generation

- [x] 2.1 Implement kind inference, compatibility checks, and the shared
  reference-widening helper for `ВЫБОР`, `ЕСТЬNULL`, and `ОБЪЕДИНИТЬ`.
- [x] 2.2 Render `CASE`, `COALESCE`, and `LIKE` on both dialects; reject
  `ПОДОБНО` outside predicate positions.
- [x] 2.3 Make `compile_predicate` and the projection date correction
  kind-driven.
- [x] 2.4 Compile aggregates over expressions with the documented kind rules.

## 3. Verification and documentation

- [x] 3.1 Add lexer, PostgreSQL, and MSSQL golden tests (CASE in projection
  and predicate, COALESCE with dates and references, LIKE with ESCAPE and
  negation, aggregates over CASE, UNION over CASE kinds, mismatch
  diagnostics).
- [x] 3.2 Update README, `docs/query-language-support.md`, and the REPL
  completion list.
- [x] 3.3 Run formatting, Clippy, workspace tests, rustdoc, and strict
  OpenSpec validation.
