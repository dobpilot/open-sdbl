## 1. Column kinds

- [ ] 1.1 Add `ColumnKind`, `CompiledColumn`, `QueryableColumn::kind`, and the
  catalog-type mapping with unit tests on real fixture strings.
- [ ] 1.2 Index physical tables in `MetadataSnapshot` and resolve reference
  targets to object IDs.

## 2. Native SQL generation

- [ ] 2.1 Remove textual casts from projections, scalars, and aggregates;
  keep the MSSQL year-offset correction and the `mchar`/`mvarchar` exception.
- [ ] 2.2 Emit one binary column per reference, including deferred
  presentation payloads and batch lookups.
- [ ] 2.3 Diagnose UNION kind mismatches before execution.
- [ ] 2.4 Update golden SQL tests and add kind assertions for both backends.

## 3. CLI

- [ ] 3.1 Introduce typed cells with PostgreSQL binary-protocol decoders and
  MSSQL `ColumnData` decoding.
- [ ] 3.2 Resolve deferred presentations from raw reference bytes.
- [ ] 3.3 Render cells with the shared formatting policy and cover it with
  unit tests.

## 4. Verification and documentation

- [ ] 4.1 Document `ColumnKind`, the reference column layout, and CLI output
  formats in README and rustdoc.
- [ ] 4.2 Run formatting, Clippy, workspace tests, rustdoc, and strict
  OpenSpec validation.
