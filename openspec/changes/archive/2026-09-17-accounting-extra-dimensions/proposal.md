## Why

The accounting tables of the demo Бухгалтерия предприятия corpus stop
on extra dimensions: the `Субконто<N>` fields and the `Субконто`
argument of `Остатки`, `Обороты` and `ОстаткиИОбороты` (31 queries), the
`Субконто` table (8), and `ЗНАЧЕНИЕ(ПланВидовХарактеристик.…)` naming
a kind of extra dimension. Measured on that base: the main table
carries the values inline (`_ValueDt<k>_*`, `_KindDt<k>RRef` per side
and level), the `_AccRgED` table keeps one row per record, side and
level, and a chart of characteristic types keeps its predefined items
in `<guid>.7`.

## What Changes

- The decoder SHALL read `<guid>.7` predefined items of a chart of
  characteristic types the way it reads `.1c` and `.9`, both providers
  SHALL fetch the resource, and `ЗНАЧЕНИЕ` SHALL accept a chart of
  characteristic types.
- `AccRgED` SHALL be a resolved service kind owned by its register, and
  `РегистрБухгалтерии.X.Субконто` (`ExtDimensions`) SHALL compile as
  that table with `ВидДвижения` (`Correspond`), `Вид`, `Значение`,
  `Период`, `Регистратор`, `НомерСтроки`, `УточнениеПериода`.
- The three aggregating tables SHALL expose `Субконто<k>` (a value of
  several types) and `ВидСубконто<k>` for `k` up to the register's level
  count, read from the side's inline columns — positional by the
  account's own extra-dimension order when the `Субконто` argument is
  omitted — as dimensions summed away when unread; `Условие` SHALL see
  them too.
- With the `Субконто` argument — one kind (`ЗНАЧЕНИЕ(ПланВидовХарактеристик.…)`
  or a parameter bound to a reference) or a parenthesized list of them
  — `Субконто<j>` SHALL take the record's value at whichever level
  carries the `j`-th listed kind, and records whose account lacks one of
  the listed kinds SHALL be excluded, as the platform documents.
- The argument layout of a register with extra dimensions is the
  platform's full one, already in place.

## Capabilities

### Modified Capabilities

- `onec-metadata`: `.7` predefined items; the `AccRgED` kind.
- `query-repl`: the `Субконто` table, the `Субконто<k>` fields and
  argument.

## Impact

`src/metadata/config.rs`, `db_names.rs`, `queries.rs`, `resolve.rs`;
`src/query/core/resolve.rs`, `codegen/accounting.rs`,
`codegen/sources.rs`; `tools/corpus/fetch_base.py`;
`docs/query-language-support.md`. `MetadataKind` and
`PredefinedSource` gain a variant each.
