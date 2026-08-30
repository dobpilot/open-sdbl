## 1. Workspace boundary

- [x] 1.1 Declare the workspace and create `open-sdbl-cli` with an `open-sdbl` binary depending on the root library.
- [x] 1.2 Remove the root binary target and move lex/metadata CLI tests to the application package.
- [x] 1.3 Verify the root `open-sdbl` package retains no production dependencies or I/O entry points.

## 2. Library query and decoding API

- [x] 2.1 Move fixed PostgreSQL metadata query definitions into the library and document their authoritative resource roles.
- [x] 2.2 Keep typed live-table records and all Config/DBNames/SchemaStorage decoding and resolution in the library.
- [x] 2.3 Add tests proving query definitions are SELECT-only and core conformance fixtures remain database-independent.

## 3. Async PostgreSQL CLI

- [x] 3.1 Replace the psql subprocess with `tokio-postgres` acquisition in one read-only `READ COMMITTED` transaction.
- [x] 3.2 Implement `PGPASSWORD`, `PGPASSFILE`, and default `.pgpass` lookup with escaping, wildcard, and Unix permission checks.
- [x] 3.3 Preserve CLI help, argument diagnostics, report output, and connection-error redaction while removing `--psql`.

## 4. Verification

- [x] 4.1 Run the new CLI against `192.168.166.15/test` and verify the known catalog and attribute mapping.
- [x] 4.2 Update README and CI/build instructions for the workspace package boundary.
- [x] 4.3 Run formatting, Clippy with warnings denied, all tests, rustdoc with warnings denied, and strict OpenSpec validation.
