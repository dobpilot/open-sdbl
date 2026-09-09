# Change: Add a read-only Microsoft SQL Server provider

## Why

`open-sdbl-cli` can currently acquire 1C metadata and execute bounded SDBL only
against PostgreSQL. Many production 1C information bases use Microsoft SQL
Server, so the same metadata navigation and read-only console need a native TDS
path without adding I/O dependencies to the reusable core crate.

## What Changes

- Add `metadata mssql` and `console mssql` CLI providers using SQL Server
  authentication over TDS.
- Add fixed SELECT-only SQL Server metadata acquisition queries for `Params`,
  `Config`, `SchemaStorage`, `_YearOffset`, and the `dbo` system catalog.
- Add an MSSQL SQL-generation dialect to the existing bounded SDBL compiler;
  reuse the parser, metadata resolver, and presentation-plan protocol.
- Preserve TLS certificate validation by default, with an explicit opt-in for
  trusting a server certificate, and read passwords from `MSSQL_PASSWORD`.
- Keep PostgreSQL commands and public compilation APIs compatible.

## Impact

- Affected specs: `crate-architecture`, `onec-metadata`, `query-repl`.
- Affected code: core metadata queries and query renderer; CLI connection,
  metadata acquisition, row rendering, tests, dependencies, and README.
- New application dependencies: Tiberius and Tokio compatibility adapters in
  `open-sdbl-cli` only.
