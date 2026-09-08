## 1. Lexer and parser

- [x] 1.1 Add the `УНИКАЛЬНЫЙИДЕНТИФИКАТОР`/`UUID` keyword to the lexer and
  the exhaustive keyword table test.
- [x] 1.2 Parse `UUID(<field>)` while keeping the keyword usable as a field
  name.

## 2. SQL generation

- [x] 2.1 Render the PostgreSQL and MSSQL permutations into native UUID
  types and report `ColumnKind::Uuid`.
- [x] 2.2 Diagnose non-reference arguments, literals, and source-free use.

## 3. Verification and documentation

- [x] 3.1 Add lexer, PostgreSQL, MSSQL, dereference, compound, and
  diagnostic tests, plus a GUID round-trip check.
- [x] 3.2 Document the function in README and the REPL completion list.
- [x] 3.3 Run formatting, Clippy, workspace tests, rustdoc, and strict
  OpenSpec validation.
