## 1. Syntax

- [x] 1.1 Parse a list of joins per branch with budget accounting.

## 2. SQL generation

- [x] 2.1 Register one scope per join and generalize condition validation
  to "joined source with any earlier scope".
- [x] 2.2 Emit the native join chain followed by cached reference joins;
  restrict FULL JOIN to the sole join.

## 3. Verification and documentation

- [x] 3.1 Add PostgreSQL and MSSQL goldens (three-source inner/left chain,
  mixed RIGHT then LEFT, same object under two aliases, dereference through
  the third source, ordering and filtering across scopes) and diagnostics
  tests (forward reference, FULL in a chain, ambiguous unqualified field).
- [x] 3.2 Update README and `docs/query-language-support.md`.
- [x] 3.3 Run formatting, Clippy, workspace tests, rustdoc, and strict
  OpenSpec validation.
