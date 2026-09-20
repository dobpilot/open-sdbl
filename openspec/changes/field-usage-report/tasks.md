## 1. The report

- [x] 1.1 `FieldUsage`, `FieldUse` and `FieldUsageRequest`.
- [x] 1.2 The catalog collects them; `Prepared::field_usage`.

## 2. The roles

- [x] 2.1 The clause role is set around projections, grouping, `ИМЕЮЩИЕ`,
  join conditions, `ГДЕ` and ordering.
- [x] 2.2 An aggregate argument and a computed expression refine it.

## 3. Tests

- [x] 3.1 A field only in `ГДЕ`; a field in two roles; an aggregate and an
  expression; a dereference; a tabular section.
- [x] 3.2 The generated SQL is unchanged.

## 4. Checks

- [x] 4.1 `cargo fmt --all -- --check`
- [x] 4.2 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 4.3 `cargo test --workspace`
- [x] 4.4 `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
- [x] 4.5 `openspec validate field-usage-report --strict`; archive
