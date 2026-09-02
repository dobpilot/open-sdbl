## 1. Contract

- [x] 1.1 Inspect the PostgreSQL CLI/metadata/compiler boundaries and primary
  SQL Server, 1C, and Tiberius documentation.
- [x] 1.2 Add and strictly validate MSSQL provider spec deltas.

## 2. Core

- [x] 2.1 Add fixed SELECT-only MSSQL metadata acquisition queries and tests.
- [x] 2.2 Add MSSQL compile/prepare APIs without changing PostgreSQL output.
- [x] 2.3 Render MSSQL limits, text/binary/Unicode values, expressions,
  presentations, aggregates, joins, and virtual-table SQL.
- [x] 2.4 Add dialect regression tests for representative supported queries.
- [x] 2.5 Preserve native MSSQL `timestamp`/`rowversion` projections without
  server-side conversion.
- [x] 2.6 Tokenize, validate, and compile hexadecimal binary literals for both
  SQL dialects.
- [x] 2.7 Compile MSSQL canonical and configuration-extension `X` tables as
  one relation for sources, dereferences, and presentation joins.

## 3. CLI

- [x] 3.1 Add provider-aware argument parsing, SQL authentication, TLS policy,
  read-only application intent, direct TCP, and SOCKS5 transport.
- [x] 3.2 Load and resolve 1C metadata from MSSQL with bounded asynchronous
  decoding and progress reporting.
- [x] 3.3 Execute compiler-produced SELECTs and render owned textual rows in
  the shared console.
- [x] 3.4 Preserve PostgreSQL CLI behavior and error wording where applicable.
- [x] 3.5 Render raw MSSQL rowversion bytes as hexadecimal text in the CLI.

## 4. Verification and documentation

- [x] 4.1 Add CLI and adapter unit tests plus an opt-in MSSQL integration test.
- [x] 4.2 Document connection, TLS, password, least-privilege, and examples.
- [x] 4.3 Run formatting, warnings-denied Clippy, workspace tests, rustdoc, and
  strict OpenSpec validation.
- [x] 4.4 Reproduce the rowversion query against the live MSSQL demo database.
- [x] 4.5 Reproduce a rowversion comparison with a `0x` literal against the
  live MSSQL demo database.
- [x] 4.6 Reproduce reference presentation through `_Reference18X1` on the
  live MSSQL demo database.
- [x] 4.7 Cover direct reads, dotted dereferences, `Presentation`, and
  `RefPresentation` through an MSSQL extension table in the live integration
  suite.
