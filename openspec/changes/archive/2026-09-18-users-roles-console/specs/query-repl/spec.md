## ADDED Requirements

### Requirement: Browse users, roles and their rights in the console
The console SHALL provide `\users` listing the users of the base — name,
description, operating-system login, the show-in-list, authentication
and administrative flags and the role count — `\user <имя>` showing one
user with the names of its roles, `\roles [<подстрока>]` listing the
roles of the configuration by name and synonym, `\role <имя>
[<Вид.Объект>]` listing what the role grants — every object with its
granted rights, or one object with its rights and restriction texts —
and `\rls <Вид.Объект> [<право>]` showing, for the current user or every
role when no user is set, the raw restriction of each role and the
expanded access: `не ограничено`, `запрещено` or the condition. Users
and rights SHALL be read on the first command that needs them through
the read-only query path and forgotten on `\refresh`; an unknown user,
role, object or right SHALL be reported without changing the console
state.

#### Scenario: Users listed
- **WHEN** the user enters `\users` on the УНФ demo base
- **THEN** the console prints one line per user with `Абдулов (директор)`
  holding three roles

#### Scenario: Role of a user
- **WHEN** the user enters `\user Абдулов (директор)`
- **THEN** the console prints the user's flags and the role names
  `АдминистраторСистемы`, `ПолныеПрава` and
  `ИнтерактивноеОткрытиеВнешнихОтчетовИОбработок`

### Requirement: Run allowed queries as a user
`\as <пользователь>` SHALL make that user current and `\as clear` SHALL
forget it; `\as` alone SHALL print the current user. With a current user,
every target a `РАЗРЕШЕННЫЕ` batch requests that no `\restrict` covers
SHALL take the access of the user's roles for `Чтение`, expanded against
the session parameters: no restriction when unrestricted, `ЛОЖЬ` when no
role grants the right, and the restrictions joined by `ИЛИ` otherwise.
A tabular section SHALL take its owner's access as
`Ссылка В (ВЫБРАТЬ <псевдоним>.Ссылка ИЗ <владелец> КАК <псевдоним> ГДЕ <условие>)`.
An expansion error — a session parameter without a value, an outdated
template — SHALL abort the query with the message, naming the role and
the parameter to set with `\session`.

#### Scenario: Restricted table for a user
- **WHEN** `\as Петрова (бухгалтер)` is set and a `РАЗРЕШЕННЫЕ` query
  reads a catalog a role of the user restricts
- **THEN** the query compiles with that role's expanded restriction,
  or reports the session parameter the template needs

#### Scenario: Denied table
- **WHEN** no role of the current user grants `Чтение` of the table
- **THEN** the query compiles with the restriction `ЛОЖЬ` and answers no
  rows

### Requirement: Skip unreadable Config resources
A Config resource the decoder cannot read — one that is not UTF-8 or not
the brace serialization, such as the binary `.7` resource of some charts
of characteristic types — SHALL be skipped with a warning naming it, and
the metadata SHALL be acquired from the rest.

#### Scenario: Binary predefined resource
- **WHEN** a `.7` resource starts with bytes that are not UTF-8
- **THEN** the console warns and the metadata is acquired
