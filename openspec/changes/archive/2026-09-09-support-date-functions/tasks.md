## 1. Lexer and parser

- [x] 1.1 Recognize bilingual date-function keywords without breaking their use
  as contextual identifiers.
- [x] 1.2 Parse and validate three-to-six-component `ДАТАВРЕМЯ` expressions.
- [x] 1.3 Parse `НАЧАЛОПЕРИОДА` with every supported bilingual period constant.

## 2. SQL generation

- [x] 2.1 Generate PostgreSQL constructors and every supported period boundary.
- [x] 2.2 Generate MSSQL constructors and boundaries with correct `_YearOffset`
  storage and projected-output handling.
- [x] 2.3 Support date expressions in source-free/source-backed projections,
  filters, joins, and virtual-table period arguments.

## 3. Verification and documentation

- [x] 3.1 Add lexer, PostgreSQL, MSSQL, validation, nesting, and year-offset
  regression tests.
- [x] 3.2 Document the supported syntax and Monday week-boundary limitation.
- [x] 3.3 Run formatting, Clippy, workspace tests, rustdoc, and strict OpenSpec
  validation.
