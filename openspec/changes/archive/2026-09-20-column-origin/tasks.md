## 1. The field identity

- [x] 1.1 `QueryableField::field: Option<FieldId>`, filled from the custom
  field index and from standard field names.
- [x] 1.2 `SourceScope` keeps the tabular-section name of its source.

## 2. The origin

- [x] 2.1 `ColumnOrigin` and `CompiledColumn::origin`.
- [x] 2.2 Filled at the column assembly for field projections, left empty
  for generated columns, marked for members of a composite.
- [x] 2.3 Nested tabular-section columns carry it too.

## 3. Tests

- [x] 3.1 Alias, every-field projection, tabular section, composite
  member, expression, aggregate.
- [x] 3.2 Truncation and de-duplication leave the origin alone.
- [x] 3.3 A dereference names the target object and its field.
- [x] 3.4 The generated SQL is unchanged.

## 4. Checks

- [x] 4.1 `cargo fmt --all -- --check`
- [x] 4.2 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 4.3 `cargo test --workspace`
- [x] 4.4 `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
- [x] 4.5 `cargo tree -p open-sdbl -e normal` still empty
- [x] 4.6 `openspec validate column-origin --strict`; archive
