## Why

Platform 8.2 bases run on SQL Server 2008/2008 R2 and PostgreSQL 9.0–9.3.
Metadata acquisition is already portable, but the query compiler still emits
`DATETIME2FROMPARTS` (SQL Server 2012) for `НАЧАЛОПЕРИОДА` and
`MAX(…) FILTER (WHERE …)` (PostgreSQL 9.4) for accumulation balances, and
the PostgreSQL catalog query uses `LATERAL … WITH ORDINALITY` (9.4). Users
need the same 1C queries to work on old and new servers without rewriting.

## What Changes

- Add `MsSqlDialectLevel { Sql2008, Sql2012 }` (`#[non_exhaustive]`, default
  `Sql2012`) and `MsSqlBackend::with_dialect_level`/`dialect_level`; the
  level is part of the immutable backend value and reaches every codegen
  path. `MsSqlBackend::new(year_offset)` keeps its meaning.
- Render `НАЧАЛОПЕРИОДА` on `Sql2008` with `DATEADD`/`DATEDIFF` arithmetic
  that yields the same `datetime2` values; every other statement is already
  2008-safe, and a parity test pins that the two levels differ only there.
- Emit the accumulation-balance anchor with `MAX(CASE WHEN …)` on PostgreSQL
  too, so generated PostgreSQL works from 9.0 without backend state.
- Add `PostgresMetadataQueries::SERVER_VERSION` and a `CATALOG_LEGACY` variant
  without `LATERAL`/`WITH ORDINALITY`, selected below server version 9.4.
- CLI: detect the level from `SERVERPROPERTY('ProductVersion')`, allow
  `--mssql-dialect 2008|2012` to override it, print the level at startup.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-compilation`: dialect-level-aware MSSQL rendering; portable
  PostgreSQL aggregate.
- `crate-architecture`: backend value carries the dialect level.
- `query-repl`: level detection and override.
- `onec-metadata`: PostgreSQL 9.0–9.3 catalog acquisition.

## Impact

- Public API additions only; `MsSqlBackend::default()` still means offset 0
  and the newest level.
- One PostgreSQL golden fragment changes (`FILTER` → `CASE`).
- No new dependencies.
