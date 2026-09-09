## 1. Metadata

- [x] 1.1 Add 1C UUID physical-byte conversion with tests.
- [x] 1.2 Decode verified predefined rows from `<guid>.1c` Config resources.
- [x] 1.3 Load `.1c` resources in PostgreSQL and MSSQL application adapters.
- [x] 1.4 Resolve and index catalog plus enumeration values with strict lookup errors.

## 2. Query compiler

- [x] 2.1 Tokenize and parse bilingual `ЗНАЧЕНИЕ`/`VALUE` expressions.
- [x] 2.2 Compile enumeration binary constants for PostgreSQL and MSSQL.
- [x] 2.3 Compile catalog `_PredefinedID` lookups for PostgreSQL and MSSQL.
- [x] 2.4 Add positional diagnostics for invalid syntax, kinds, names, and schema.

## 3. Verification

- [x] 3.1 Add lexer, metadata, SQL generation, and error-path unit tests.
- [x] 3.2 Verify all three requested values against PostgreSQL `erp_ur`.
- [x] 3.3 Run formatting, Clippy with warnings denied, workspace tests, rustdoc
  with warnings denied, and strict OpenSpec validation.
