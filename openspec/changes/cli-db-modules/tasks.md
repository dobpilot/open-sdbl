## 1. Split

- [x] 1.1 Split `db/postgres.rs` into `session.rs`, `metadata.rs` and
  `cells.rs` under `db/postgres/`.
- [x] 1.2 Split `db/mssql.rs` the same way.

## 2. Tests

- [x] 2.1 Move the provider tests into `src/tests/`, one file per module.
- [x] 2.2 Move every remaining unit test — arguments, parameters, pipeline,
  cells, restrictions, output, progress, errors, SOCKS5 and the password
  file — into `src/tests/`, so no implementation file carries test code.

## 3. Verification

- [x] 3.1 Run formatting, Clippy with warnings denied, workspace tests,
  rustdoc with warnings denied, the bounded fuzz checks, and strict
  OpenSpec validation.
