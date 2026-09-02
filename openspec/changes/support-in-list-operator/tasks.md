## 1. Query grammar and AST

- [x] 1.1 Add an explicit IN-list expression node.
- [x] 1.2 Parse bilingual `В`/`IN` with a non-empty scalar-expression list.
- [x] 1.3 Add positional diagnostics for malformed lists.

## 2. SQL generation

- [x] 2.1 Generate PostgreSQL IN predicates with left-operand type context.
- [x] 2.2 Generate MSSQL IN predicates with left-operand type context.
- [x] 2.3 Support IN predicates in source-free, single-source, and join contexts.

## 3. Verification

- [x] 3.1 Add parser, SQL generation, VALUE-list, and error-path tests.
- [x] 3.2 Verify the requested predicate against PostgreSQL `erp_ur`.
- [x] 3.3 Run formatting, Clippy with warnings denied, workspace tests, rustdoc
  with warnings denied, and strict OpenSpec validation.
