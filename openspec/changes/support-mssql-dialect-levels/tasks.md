## 1. Core

- [ ] 1.1 Add `MsSqlDialectLevel`, the `MsSqlBackend` builder, and thread the
  level through `SqlDialect`.
- [ ] 1.2 Render `НАЧАЛОПЕРИОДА` with `DATEADD`/`DATEDIFF` on `Sql2008`; make
  the PostgreSQL balance anchor use `CASE`.
- [ ] 1.3 Add golden tests for every period on `Sql2008`, a level parity
  test, and product-version parsing tests.

## 2. Acquisition and CLI

- [ ] 2.1 Add `SERVER_VERSION`/`CATALOG_LEGACY` for PostgreSQL and
  `PRODUCT_VERSION` for MSSQL; select the catalog variant by version.
- [ ] 2.2 Detect the MSSQL level at connection, support `--mssql-dialect`,
  print the level.
- [ ] 2.3 Add an ignored live test for `НАЧАЛОПЕРИОДА` on SQL Server 2008.

## 3. Verification and documentation

- [ ] 3.1 Document the level, the flag, and supported server versions.
- [ ] 3.2 Run formatting, Clippy, workspace tests, rustdoc, and strict
  OpenSpec validation.
