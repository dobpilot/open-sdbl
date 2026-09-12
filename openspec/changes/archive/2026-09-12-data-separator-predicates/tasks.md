## 1. Config layout

- [x] 1.1 Build a probe configuration with `ibcmd` on platform 8.3.27
  (two common attributes in `Разделять` mode, one `Независимо`, one
  `Независимо и совместно`, one bound to session parameters, one unbound),
  create an infobase on the PostgreSQL server, dump the common-attribute
  Config resources, and record the mode position and encoding in
  `design.md`.
- [x] 1.2 Add a fixture under `tests/fixtures/` with DBNames
  `DataSeparationUse` entries, the two common-attribute resources, the
  bound session parameter descriptors, and SchemaStorage tables with and
  without the separator column.

## 2. Metadata

- [x] 2.1 Project `DataSeparationSettings` from class-id-`5` resources in
  `config.rs` with unit tests on the demo layout and on a truncated tail.
- [x] 2.2 Resolve settings and session parameter names into
  `MetadataField::separation`, add `MetadataSnapshot::separators`, add the
  `SeparatorSettingsMissing` finding, and cover both in
  `tests/metadata_lookup.rs`.

## 3. SQL generation

- [x] 3.1 Resolve separator values per statement on `CompilationCatalog`
  (use flag, bound parameter, attribute-name fallback, typed empty default,
  `Independent` diagnostic).
- [x] 3.2 Return separator predicates from `compile_source_relation` per
  extension branch and place them for `ИЗ`, inner and one-sided outer
  joins, `ПОЛНОЕ` per emulated direction, dereference and presentation joins,
  nested queries, and restricted sources.
- [x] 3.3 Conjoin separator predicates into slice, balance, and turnover
  base reads.

## 4. Verification and documentation

- [x] 4.1 Goldens on both dialects: plain source, each join kind, extension
  `UNION ALL` with a branch lacking the column, dereference, slice and
  balance, nested `В (ВЫБРАТЬ …)`, restricted source, disabled separator,
  default value per type, `Independent` diagnostic, base without
  separators byte-identical.
- [x] 4.2 Run the console against the demo base: `ВЫБРАТЬ … ГДЕ Ссылка =
  &Ссылка` and a reference join with `\session ОбластьДанныхЗначение = 0`
  and confirm the PostgreSQL plan seeks the primary key.
- [x] 4.3 Update README and `docs/query-language-support.md`; run
  formatting, Clippy, workspace tests, rustdoc, and strict OpenSpec
  validation.
