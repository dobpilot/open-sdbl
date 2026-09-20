## Why

`DatabaseSession::query` materializes a whole result, twice.

On PostgreSQL `transaction.query(sql, &[])` is `query_raw(…).try_collect()`:
every row lands in a `Vec<Row>`, and the decode pass then builds
`Vec<Vec<Cell>>` while that vector is still alive. On SQL Server it is
worse — `into_first_result()` calls `into_results()`, which collects
*every* result set before the first one is taken.

Nothing on that path bounds the number of rows or bytes. The only limit in
the tree is the console's `.take()` at print time, after all the copies.
A query over ten million rows exhausts the process before anything else
reacts.

Cancellation is the other half. `query_timeout` and
`bounded_database_call` are `tokio::time::timeout`: they drop the future
and tell the server nothing. PostgreSQL is saved by the server-side
`statement_timeout` set at connect; SQL Server sets only `LOCK_TIMEOUT`,
which bounds lock waiting, not execution. A SQL Server query that runs too
long keeps running.

## What Changes

- A session SHALL be able to read a result **row by row**, decoding one row
  at a time, and the consumer SHALL be able to stop at any point without
  reading the remainder.
- Stopping SHALL not read, decode, or allocate the rows that follow.
- On SQL Server, stopping early SHALL end the statement on the server:
  the connection carrying it is dropped rather than drained, which is the
  cancellation that provider otherwise lacks.
- `DatabaseSession::query` SHALL remain, with its present signature and
  behaviour, as a consumer of the streaming path, so the console and the
  tests are untouched.

## Capabilities

### Modified Capabilities

- `crate-architecture`: how a session delivers a result, and what stopping
  early guarantees.

## Impact

- `crates/open-sdbl-db/src/session.rs`, `db/postgres/session.rs`,
  `db/mssql/session.rs`, `cells.rs`.
- New unit tests over a synthetic row source, and one live SQL Server test.
- No new production dependency; the core crate is untouched.
