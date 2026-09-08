## 1. Indexing

- [x] 1.1 Index live tables, extension variants, schema tables, and
  extension fields in the snapshot and expose lookups.
- [x] 1.2 Replace linear scans in field projection, source resolution,
  dereference joins, presentation lookups, and relation compilation.

## 2. Verification

- [x] 2.1 Add a regression test compiling a join with dereferenced
  presentations against a snapshot inflated to 20,000 tables.
- [x] 2.2 Run formatting, Clippy, workspace tests, rustdoc, and strict
  OpenSpec validation.
