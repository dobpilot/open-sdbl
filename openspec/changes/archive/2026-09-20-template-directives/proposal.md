## Why

The manual (5.5.4.8.7) lists what may follow `#` in a template: a
numbered parameter, `ТекущаяТаблица`, `ИмяТекущейТаблицы`,
`ИмяТекущегоПраваДоступа`, a named parameter of the signature, and `#`
itself, which stands for one `#` in the text. Two of those the library
gets wrong: it does not read `##` at all, and it inserts the table name
of `#ИмяТекущейТаблицы` bare, where the manual says it stands for the
name **as a string value, in quotes**.

## What Changes

- `##` SHALL stand for one `#`, and SHALL be read as text wherever a
  directive, a call or a parameter is looked for.
- `#ИмяТекущейТаблицы` SHALL insert the name of the table as a quoted
  string, while `#ТекущаяТаблица` inserts it bare, as the manual says.
- `RestrictionTemplate` SHALL be constructible, so an application can
  expand a text against templates of its own.

## Capabilities

### Modified Capabilities

- `access-rights`: the directives of a template text.

## Impact

`src/access.rs`, `src/metadata/roles.rs`, `tests/access_templates.rs`.
