## Why

`ОборотыДтКт` and `ДвиженияССубконто` are the two accounting tables
left: the demo Бухгалтерия предприятия corpus reads them in 10 and 8
queries — the correspondence of accounts with the extra dimensions of
both sides, and the records with their extra dimensions.

## What Changes

- `ОборотыДтКт(Начало, Конец, Периодичность, УсловиеСчетаДт, СубконтоДт,
  УсловиеСчетаКт, СубконтоКт, Условие)` (the `Субконто` arguments absent
  without extra dimensions) SHALL answer one row per pair of accounts,
  the balance dimensions, both sides of the non-balance ones, both sides'
  extra dimensions in use and the split, over the active records of
  `[Начало, Конец)`: per balance resource `<Ресурс>Оборот`, per
  non-balance one `<Ресурс>ОборотДт` and `<Ресурс>ОборотКт`; unread
  dimensions are summed away; the record is not folded. Each side's
  listed kinds map that side's `Субконто<j>` and exclude the records
  whose account on that side lacks a kind. The three conditions see the
  record as the main table shows it.
- `ДвиженияССубконто(Начало, Конец, Условие, Порядок, Первые)` SHALL
  answer the records of `[Начало, Конец)` with the main table's fields
  and `СубконтоДт<k>`, `ВидСубконтоДт<k>`, `СубконтоКт<k>`,
  `ВидСубконтоКт<k>` from the inline columns; `Порядок` and `Первые`
  SHALL be `UnsupportedFeature` diagnostics for now.

## Capabilities

### Modified Capabilities

- `query-repl`: the two record-level accounting tables.

## Impact

`src/query/core/codegen/accounting.rs`; `docs/query-language-support.md`.
