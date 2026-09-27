## Implementation

- [x] Add opt-in sequential materialized-batch API with typed host callback and result form.
- [x] Validate and store selected rows, including empty tables, dates, references, `NULL`, appends, drops, and bounded memory.
- [x] Preserve legacy `compile_batch` behavior and version all workspace crates as 0.9.4.
- [x] Add PostgreSQL and SQL Server tests for snapshot, append, drop, dependencies, failure rollback, and unsupported rows.
- [x] Run formatting, Clippy, workspace tests, rustdoc, and strict OpenSpec validation.
