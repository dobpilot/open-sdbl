## Why

The console reports SDBL-to-SQL generation time but does not distinguish it
from time spent executing the generated statement in PostgreSQL.

## What Changes

- Measure the PostgreSQL query future independently from compilation and row
  rendering.
- Display PostgreSQL execution duration after every successful query.
- Include elapsed execution time in a recoverable PostgreSQL query failure.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: expose PostgreSQL statement execution duration.

## Impact

- CLI-only output change; database access and the core library API are
  unchanged.
