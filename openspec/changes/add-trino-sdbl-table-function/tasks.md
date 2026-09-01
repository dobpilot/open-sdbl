## 1. Contract

- [x] 1.1 Add and strictly validate the SDBL table-function OpenSpec delta.
- [x] 1.2 Document the exact Trino SQL invocation and safety boundary.

## 2. Rust service

- [x] 2.1 Share the default presentation-plan policy with application crates.
- [x] 2.2 Add SDBL prepare metadata and scan wire models.
- [x] 2.3 Add prepare and streaming scan endpoints with shape validation,
  projection, limit, timeouts, and read-only execution.
- [x] 2.4 Add compiler, type, shape, and SQL-wrapper tests.

## 3. Trino 476 plugin

- [x] 3.1 Register `system.sdbl` as a polymorphic connector table function.
- [x] 3.2 Analyze through the Rust prepare endpoint and implement
  `applyTableFunction` with serializable handles.
- [x] 3.3 Reuse the split/record path for SDBL scans and retain projection and
  limit pushdown.
- [x] 3.4 Add Java model and table-function tests.

## 4. Verification

- [x] 4.1 Verify presentation, SliceLast, and Balance queries against the local
  PostgreSQL fixture through stock Trino 476.
- [x] 4.2 Run Rust formatting, warnings-denied Clippy, workspace tests, rustdoc,
  Java tests/package, and strict OpenSpec validation.
