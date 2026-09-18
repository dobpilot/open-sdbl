## Why

A restriction of a role is not yet a condition the compiler applies: it
is a text with preprocessor directives (`#Если … #Тогда … #ИначеЕсли …
#Иначе … #КонецЕсли`), template calls (`#ДляОбъекта("")`) whose bodies
lie in the same role, and the platform's full restriction form
`ТекущаяТаблица ГДЕ …` that names the restricted table inside nested
queries. The user's roles then combine: any role granting the right
grants it, and their restrictions join with `ИЛИ`.

## What Changes

- `access::expand_restriction` SHALL expand a restriction text with the
  role's templates against the session parameters: template calls with
  positional and named parameters, `#ИмяТекущейТаблицы`,
  `#ИмяТекущегоПраваДоступа`, directives whose expressions compare
  strings and booleans, concatenate and call `СтрСодержит`; a session
  parameter without a value SHALL be an error naming it; a result
  `Ошибка: …` or another labelled message SHALL be an error carrying it.
- `access::read_access` SHALL combine the roles of a user for one object
  and right: `Denied` when no role grants it, `Unrestricted` when one
  grants it without restriction, otherwise the expanded restrictions,
  joined by `ИЛИ` into one condition.
- The compiler SHALL accept a restriction condition in the platform's
  full form `ТекущаяТаблица [КАК <псевдоним>] ГДЕ <условие>` and SHALL
  resolve `ТекущаяТаблица` and the alias as qualifiers of the restricted
  table, inside nested queries too; a join in the restriction SHALL be
  refused with a `Restriction` diagnostic.

## Capabilities

### Modified Capabilities

- `access-rights`: restriction expansion and the access of a user.
- `query-compilation`: the full form of a restriction condition.

## Impact

`src/access.rs` (new, `open_sdbl::access`), `compile_restriction_predicate`
in `src/query/core/codegen/sources.rs`, `SourceScope::is_qualifier`.
