## 1. Syntax

- [x] 1.1 Add the `ВЫРАЗИТЬ`/`CAST` keyword and parse scalar and reference
  targets with an optional trailing field.

## 2. SQL generation

- [x] 2.1 Render scalar casts on both providers with the target column kind.
- [x] 2.2 Compile reference narrowing values and type-guarded dereferences.
- [x] 2.3 Render boolean predicates as comparisons on MSSQL.

## 3. Verification and documentation

- [x] 3.1 Add lexer, PostgreSQL, MSSQL, narrowing, diagnostic, and predicate
  tests.
- [x] 3.2 Document the syntax in README and complete both spellings in the
  REPL.
- [x] 3.3 Run formatting, Clippy, workspace tests, rustdoc, and strict
  OpenSpec validation.
