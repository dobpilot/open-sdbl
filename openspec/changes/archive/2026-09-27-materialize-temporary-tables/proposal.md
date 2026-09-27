## Why

The existing `TempTablesManager` retains a CTE for each placement. A later read reruns the source query. On measured 1C 8.3.27.2342, a temporary table retains the value from `ПОМЕСТИТЬ` even after the source row changes. The compiler needs an opt-in host execution path that stores the rows selected at placement time.

## What Changes

- Add a sequential materialized-batch API to the compiler and prepared query. It asks the host to execute each placement selection once and return typed rows.
- Store validated rows as read-only SQL literals in the manager. Appends extend the stored rows; drops hide them. Later statements use these rows without reading the source again.
- Retain the existing CTE-only `compile_batch` contract for existing callers.
- Bump workspace crate versions to 0.9.4.

## Capabilities

### Modified Capabilities

- `query-repl`: opt-in materialized temporary-table execution.

## Impact

- Root query compiler, temporary-table manager, SQL rendering and tests for PostgreSQL and SQL Server.
- No database I/O or new production dependency in the root crate.
