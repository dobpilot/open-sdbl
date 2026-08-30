## Context

The console calls `PostgresSession::query` after compilation. That method also
starts, verifies, commits, or rolls back a read-only transaction.

## Decisions

### Time the PostgreSQL query call at the console boundary

Start the execution timer immediately before `session.query` and stop it when
the future returns. This reports the end-to-end database operation visible to
the console, including read-only transaction setup and commit, but excludes
SQL generation and result formatting.

### Reuse compact duration formatting

Use the same nanosecond/microsecond/millisecond formatter as SQL generation so
the two measurements are directly comparable.
