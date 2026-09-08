## 1. Column kinds

- [x] 1.1 Add `ColumnKind`, `CompiledColumn`, `QueryableColumn::kind`, and the
  catalog-type mapping with unit tests on real fixture strings.
- [x] 1.2 Index physical tables in `MetadataSnapshot` and resolve reference
  targets to object IDs.

## 2. Native SQL generation

- [x] 2.1 Remove textual casts from projections, scalars, and aggregates;
  keep the MSSQL year-offset correction and the `mchar`/`mvarchar` exception.
- [x] 2.2 Emit one binary column per reference, including deferred
  presentation payloads and batch lookups.
- [x] 2.3 Diagnose UNION kind mismatches before execution.
- [x] 2.4 Update golden SQL tests and add kind assertions for both backends.

## 3. CLI

- [x] 3.1 Introduce typed cells with PostgreSQL binary-protocol decoders and
  MSSQL `ColumnData` decoding.
- [x] 3.2 Resolve deferred presentations from raw reference bytes.
- [x] 3.3 Render cells with the shared formatting policy and cover it with
  unit tests.

## 4. Verification and documentation

- [x] 4.1 Document `ColumnKind`, the reference column layout, and CLI output
  formats in README and rustdoc.
- [x] 4.2 Run formatting, Clippy, workspace tests, rustdoc, and strict
  OpenSpec validation.
