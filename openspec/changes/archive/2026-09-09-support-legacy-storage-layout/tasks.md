## 1. Core queries

- [x] 1.1 Add `StorageLayout`, the layout probe queries, and legacy query
  variants with a shared row shape; keep every statement in `all()`.
- [x] 1.2 Unit-test the SELECT-only audit, the absence of `PartNo` in legacy
  variants, and the probe shape.

## 2. CLI acquisition

- [x] 2.1 Detect the layout after the transaction starts, reject bases
  without `SchemaStorage`, and pass the layout to every reader.
- [x] 2.2 Assemble multi-part resources with a shared reader and cover it
  with unit tests.
- [x] 2.3 Skip extension queries when their tables are absent.

## 3. Verification and documentation

- [x] 3.1 Add an ignored live test for a legacy MSSQL base.
- [x] 3.2 Document supported platform layouts and multi-part assembly.
- [x] 3.3 Run formatting, Clippy, workspace tests, rustdoc, and strict
  OpenSpec validation.
