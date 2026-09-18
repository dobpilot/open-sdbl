## Why

An unaliased projection of a standard field was labelled by the schema
name — `ID`, `Code`, `Recorder`, `LineNo` — while the platform names the
column as the text spells it (`Ссылка`, `Код`, `Регистратор`); a
temporary table defined by `ВЫБРАТЬ Т.Регистратор ПОМЕСТИТЬ ВТ` could
then not be read as `ВТ.Регистратор`, which seven queries of the demo
Бухгалтерия corpus do.

## What Changes

- An unaliased field projection SHALL be labelled by the last segment
  of the path as written, with the member suffixes of a compound field
  appended as for an alias; an aliased projection keeps its alias.

## Capabilities

### Modified Capabilities

- `query-repl`: result labels of unaliased fields.

## Impact

`select.rs` (`path_label` of an unaliased field); recorded labels of
`Ссылка`, `Код`, `Регистратор`, `НомерСтроки`, … change in the goldens
and the corpora; the two-sided completion method row is unrelated.
