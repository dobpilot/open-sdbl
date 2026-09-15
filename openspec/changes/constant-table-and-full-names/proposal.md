## Why

Two shapes real configurations use are refused, and one of them is worse
than refused — it answers under a name the platform does not accept.

`ИЗ Константа.<Имя>` compiles today, but its value field is named after
the constant, so `К.ОсновнойТовар` resolves and `К.Значение` does not.
Measured on the probe base against 8.3.27, the platform answers exactly the
other way round: `ВЫБРАТЬ * ИЗ Константа.ОсновнойТовар` yields the columns
`Значение` and the common attributes, and `К.ОсновнойТовар` fails with
"Поле не найдено". The corpus query that reads three constants through
`Константа.<Имя>` stops on this.

A source written without an alias can be addressed by its full metadata
name: measured, `ВЫБРАТЬ Справочник.Товары.Наименование ИЗ
Справочник.Товары` answers, and so does `Документ.Продажа.Номер`. With an
alias the full name is refused by the platform — "Поле не найдено
'Справочник.Товары.Цена'" — because the alias replaces the name. The
compiler refuses both, reporting `unknown source qualifier`.

## What Changes

- Name the value field of a `Константа.<Имя>` source `Значение`, accepting
  the English `Value`, instead of naming it after the constant.
- Resolve a field qualified by the full metadata name of a source that has
  no alias, and keep refusing it when the source has one.

## Capabilities

### Modified Capabilities

- `query-repl`: a constant table answers under the field name the platform
  uses, and an unaliased source can be qualified by its full name.

## Impact

Generated SQL is unchanged; what changes is which names resolve. A query
that read a constant table by the constant's own name now fails, which
matches the platform.
