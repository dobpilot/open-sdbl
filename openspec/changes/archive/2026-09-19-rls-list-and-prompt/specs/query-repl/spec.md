## MODIFIED Requirements

### Requirement: Browse users, roles and their rights in the console
The console SHALL provide `\users` listing the users of the base — name,
description, operating-system login, the e-mail, the show-in-list,
authentication and administrative flags and the role count — `\user
<имя>` showing one user with the names of its roles, `\roles
[<подстрока>]` listing the roles of the configuration by name and
synonym, `\role <имя> [<Вид.Объект>]` listing what the role grants —
every object with its granted rights, or one object with its rights and
restriction texts — and `\rls <Вид.Объект> [<право>]` showing, for the
current user or every role when no user is set, the raw restriction of
each role and the expanded access: `не ограничено`, `запрещено` or the
condition. `\rls` without an object SHALL list the restrictions of the
rights already read — the role, the object and the right of each, and
for a current user only the roles of that user — reading nothing, and
SHALL say which command reads rights when none are read yet. Users and
rights SHALL be read on the first command that needs them through the
read-only query path and forgotten on `\refresh`; an unknown user, role,
object or right SHALL be reported without changing the console state.

#### Scenario: Users listed
- **WHEN** the user enters `\users` on the УНФ demo base
- **THEN** the console prints one line per user with `Абдулов (директор)`
  holding three roles

#### Scenario: Role of a user
- **WHEN** the user enters `\user Абдулов (директор)`
- **THEN** the console prints the user's flags and the role names
  `АдминистраторСистемы`, `ПолныеПрава` and
  `ИнтерактивноеОткрытиеВнешнихОтчетовИОбработок`

#### Scenario: Loaded restrictions listed
- **WHEN** the user enters `\rls` after a command that read the rights of
  a restricting role
- **THEN** the console prints one line per restricted right with the role,
  the object and the right, and no query is sent

#### Scenario: Nothing read yet
- **WHEN** the user enters `\rls` before any rights are read
- **THEN** the console prints no restriction and names the commands that
  read rights

### Requirement: Run allowed queries as a user
`\as <пользователь>` SHALL make that user current and `\as clear` SHALL
forget it; `\as` alone SHALL print the current user. While a user is
current, the console prompt SHALL name it instead of `open-sdbl`, and
the continuation prompt SHALL keep its width; `\as clear` and `\refresh`
SHALL restore `open-sdbl=>`. With a current user,
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

#### Scenario: Prompt names the user
- **WHEN** `\as Абдулов (директор)` is accepted
- **THEN** the prompt reads `Абдулов (директор)=> ` until `\as clear`
