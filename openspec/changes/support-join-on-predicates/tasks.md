## 1. Join condition analysis

- [x] 1.1 Flatten top-level AND members and identify a mandatory cross-source equality.
- [x] 1.2 Validate that fields in additional predicates are direct source fields.
- [x] 1.3 Preserve compound reference equality and FULL JOIN marker handling.

## 2. SQL generation

- [x] 2.1 Compile additional scalar predicates in PostgreSQL and MSSQL ON clauses.
- [x] 2.2 Preserve native and transposed outer-join semantics.
- [x] 2.3 Add positional diagnostics for missing anchors and reference paths.

## 3. Verification

- [x] 3.1 Add tests for IN/VALUE predicates, dialects, FULL JOIN, and errors.
- [x] 3.2 Verify the reported query shape against PostgreSQL `erp_ur`.
- [x] 3.3 Run formatting, Clippy with warnings denied, workspace tests, rustdoc
  with warnings denied, and strict OpenSpec validation.
