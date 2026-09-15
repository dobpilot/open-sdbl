## Why

The MSSQL session still asks the server for `@@TRANCOUNT` before every
read and refuses to continue when it is not zero. That is one round trip
per query spent on a state the CLI itself controls: it opens its own
transaction, rolls it back, and poisons the session when the rollback
fails. The check adds latency and a failure mode of its own without
preventing anything the compiler could do — it generates `SELECT`
statements only.

## What Changes

- Drop the per-read session verification on MSSQL entirely.
- Keep the recovery path: a failed rollback still poisons the session, so
  a session left mid transaction is never reused.

## Capabilities

### Modified Capabilities

- `query-repl`: sessions are kept read-only by construction, not by
  probing the server before each read.

## Impact

One fewer round trip per MSSQL query and one fewer way to be refused. The
surrounding transaction, its rollback and the poisoning on failure are
unchanged.
