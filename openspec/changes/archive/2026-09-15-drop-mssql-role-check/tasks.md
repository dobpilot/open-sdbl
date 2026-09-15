## 1. Session

- [x] 1.1 Replace the role and isolation verification with a check that no
  transaction is already open.
- [x] 1.2 Remove the verification statement and the tests that assert it.

## 2. Documentation

- [x] 2.1 Update `README.md` and `CLAUDE.md`: a read-only login is a
  recommendation, not a requirement the CLI enforces.

## 3. Verification

- [x] 3.1 Run formatting, Clippy with warnings denied, workspace tests,
  rustdoc with warnings denied, the bounded fuzz checks, and strict
  OpenSpec validation.
