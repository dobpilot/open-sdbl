## Why

The platform's full form of a restriction names the restricted table and
then describes it: `ТекущаяТаблица ИЗ Справочник.Файлы КАК
ТекущаяТаблица ГДЕ …`. A template of «1С:Документооборот» writes exactly
that, through the directive `#ТекущаяТаблица`, which the library does not
substitute either. Both together refuse every restriction of that
configuration — 114 objects of the demo base for one user — with
«a restriction joining other tables is not supported ("ИЗ" after
ТекущаяТаблица)», although the text joins nothing: its joins are inside a
subquery of the condition.

## What Changes

- `#ТекущаяТаблица` SHALL stand for the name of the restricted table, as
  `#ИмяТекущейТаблицы` does, in the text and in a directive expression.
- The expansion SHALL read the source description of the full form —
  `ИЗ <таблица> [КАК <псевдоним>]` — when it names the restricted table,
  take the alias from it, and answer the condition in the simple form the
  compiler already takes. A description naming another table, or
  carrying more than one source, SHALL stay unsupported, with a message
  naming what was found.

## Capabilities

### Modified Capabilities

- `access-rights`: the source description of a restriction text.

## Impact

`src/access.rs` (`substitute_names`, `tokenize_expression`, `parse_form`).
