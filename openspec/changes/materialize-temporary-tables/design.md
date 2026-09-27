## Decision

Provide an opt-in sequential compilation method accepting a host callback. The compiler parses once, compiles one statement at a time, and invokes the callback once for every `ПОМЕСТИТЬ` or `ДОБАВИТЬ` selection. The callback returns logical typed scalar values in projected-column order. Its result is validated against the compiled columns and rendered as a bounded literal relation. The result of a final placement is the number of callback rows, not a second SQL count query. A final ordinary selection returns SQL for the host to execute; a final drop returns no result.

The complete batch is compiled against a private manager before the first callback, so a missing query or session parameter (or a later invalid statement) is diagnosed without reading an earlier placement twice. The manager is cloned for the sequence and committed only when compilation and callbacks succeed. This preserves the existing all-or-nothing manager state contract; SQL reads already performed cannot be undone. A callback error propagates. No transaction or READ COMMITTED snapshot promise is made across distinct statements.

`ДОБАВИТЬ` validates the same positional structure as the existing path, then appends the new rows to the previous materialized rows. It rejects a previous CTE-only definition, because adding to it would rerun the source. `УНИЧТОЖИТЬ` hides the table, and subsequent resolution fails until a new placement with that name. Dependencies use already materialized entries and cannot trigger a source read.

This path stores values in the manager and generates SQL literals; callers must bound the size of rows returned by their host callback. The compiler also imposes a fixed manager byte limit and returns a structured temporary-table diagnostic before admitting rows that exceed it. Unsupported kinds and nested parameter values fail closed. PostgreSQL and SQL Server use the existing parameter-literal renderer; date values passed back by the host are logical dates, converted to the storage domain when stored.

The legacy `compile_batch` keeps its CTE behavior and is explicitly not a materialization path, avoiding a silent semantic change for existing applications. A future host can migrate to the opt-in API.

For SQL Server, the defining statement is normally compiled in the physical date domain. The callback SQL projects each date column into the logical 1C date domain; the retained literal renderer applies `_YearOffset` when it reconstructs the storage-domain CTE. The database driver itself does not shift `Cell::DateTime`.
