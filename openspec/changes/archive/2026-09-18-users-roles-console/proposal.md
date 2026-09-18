## Why

The library now decodes users, roles, rights and restriction texts; the
console still takes restrictions only by hand through `\restrict`. An
operator wants to see who the users of a base are, which roles they
hold, what a role grants, what restriction a table gets for a user — and
to run `ВЫБРАТЬ РАЗРЕШЕННЫЕ` as that user.

## What Changes

- The metadata snapshot SHALL carry the role identifiers of the
  configuration, so `RoleCatalog::from_snapshot` names the roles without
  another read; the acquisition keeps the storage layout for later reads.
- The console SHALL provide `\users`, `\user <имя>`, `\roles
  [<подстрока>]`, `\role <имя> [<Вид.Объект>]`, `\rls <Вид.Объект>
  [<право>]` and `\as <пользователь> | clear`; users and rights are read
  on the first command that needs them, through the read-only query
  path, and forgotten on `\refresh`.
- With a current user, every restriction target of a `РАЗРЕШЕННЫЕ` batch
  that `\restrict` does not cover SHALL take the access of the user's
  roles for `Чтение`: nothing when unrestricted, `ЛОЖЬ` when denied, the
  expanded restrictions joined by `ИЛИ` otherwise; a tabular section
  takes its owner's access through `Ссылка В (ВЫБРАТЬ …)`; an expansion
  error aborts the query with the message.
- A right a rights resource does not list SHALL follow the role's
  `setForNewObjects` default, which is how `ПолныеПрава` grants reading
  while listing only its refusals.
- `object_query_name` SHALL spell a metadata object the way a query
  names it, for `#ИмяТекущейТаблицы` and the listings.

## Capabilities

### Modified Capabilities

- `query-repl`: the console commands and the user-derived restrictions;
  unreadable Config resources are skipped with a warning.
- `access-rights`: rights not listed follow the role default.
- `onec-metadata`: the snapshot carries the roles; the object query name.

## Impact

`crates/open-sdbl-cli/src/access.rs` (new), the console loop,
`pipeline.rs` (roles and layout), `MetadataSnapshot::attach_roles`,
`open_sdbl::query::object_query_name`. Version 0.5.5.
