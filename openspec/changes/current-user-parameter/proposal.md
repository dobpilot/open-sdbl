## Why

The restrictions of «1С:Документооборот» and of the Standard Subsystems
Library compare rows with `&ТекущийПользователь` — the element of
`Справочник.Пользователи` of the user whose session runs the query. `\as`
knows that user, and the catalog carries its information-base identifier,
so the console can answer the parameter itself instead of failing every
such restriction.

## What Changes

- `\as <пользователь>` SHALL read the element of
  `Справочник.Пользователи` whose `ИдентификаторПользователяИБ` is the
  identifier of that user and store it as the session parameter
  `ТекущийПользователь`, unless one is already stored.
- A configuration without that catalog, or without a row for the user,
  SHALL be no error.

## Capabilities

### Modified Capabilities

- `query-repl`: `\as` answers the current-user parameter.

## Impact

`crates/open-sdbl-cli/src/access_cache.rs`,
`crates/open-sdbl-cli/src/access.rs`.
