## 1. Resolution

- [x] 1.1 Accept `Константы`/`Constants` as a source in the parser and
  resolver, build its field catalog from live constants, and add the
  `RecordKey` recase token.
- [x] 1.2 Track which constants a statement references (projection, `*`,
  predicates, dereferences, ordering, nested use).

## 2. SQL generation

- [x] 2.1 Render the `UNION ALL` plus `MAX` derived table with `NULL`
  placeholders per branch and the statement's separator predicates.
- [x] 2.2 Map output column kinds per constant and raise
  `UnsupportedFeature` for a referenced constant whose separator is
  disabled.

## 3. CLI

- [x] 3.1 Offer `Константы` in completion and `\d Константы` listing the
  constants with their kinds.

## 4. Verification and documentation

- [x] 4.1 Goldens on both dialects: two constants, `*`, reference constant
  with dereference and `ПРЕДСТАВЛЕНИЕ`, constants table joined with a
  catalog, nested query, separated base, disabled-separator diagnostic,
  unknown constant name diagnostic.
- [x] 4.2 Run `ВЫБРАТЬ * ИЗ Константы` on the PostgreSQL demo base (562
  constants, one row, 1.45 s) and a typed selection with a reference
  constant, its dereference, and `ПРЕДСТАВЛЕНИЕ`; the MSSQL demo base was
  not reachable in this session (no `MSSQL_PASSWORD`), so `MAX` over
  `binary(1)` booleans and `varbinary(max)` stays unverified there.
- [x] 4.3 Update README and `docs/query-language-support.md`; run
  formatting, Clippy, workspace tests, rustdoc, and strict OpenSpec
  validation.
