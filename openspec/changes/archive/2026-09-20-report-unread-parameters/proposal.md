## Why

`\as` reads the session parameters of the restriction templates from
`ПараметрыОграниченияДоступа`, and says nothing when it cannot. On a base
whose register carries a data separator, the read needs the separator's
session parameter, which the operator has not set yet — so the values are
silently missing, and the templates then fail with their own message
(«Требуется обновить шаблон … Используется устаревшая версия 9 шаблона»),
which names neither the register nor the parameter to set.

Measured beside it: SQL Server keeps the value of that column in the
column itself, where PostgreSQL keeps a reference to the parts stored
apart. The reader takes only the reference, so even with the separator
set it reads nothing.

## What Changes

- `\as` SHALL say when the base carries the register and its values could
  not be read, with the reason, so the operator can set what the read
  needs and run `\as` again. A configuration without the register SHALL
  stay silent.
- A `ХранилищеЗначения` column that carries the value itself SHALL be
  decoded as it stands, beside the one that carries a reference.

## Capabilities

### Modified Capabilities

- `onec-metadata`: a stored value the column carries itself.
- `query-repl`: the report of `\as` about the parameters of the base.

## Impact

`crates/open-sdbl-cli/src/access_cache.rs`,
`crates/open-sdbl-cli/src/access.rs`.
