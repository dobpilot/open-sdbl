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
SHALL say which command reads rights when none are read yet.

`\role <имя>` without an object SHALL also list the restriction templates
of the role by signature — the name with its parameter names — and
`\template <роль> [<имя>]` SHALL show those templates with the size of
each body and whether it parses, or, with a template name, the body of
that one template as the role records it, preceded by what the body is
made of: the number of conditions, the calls by template name, and the
parameters the body reads. A template name the role does not carry SHALL
be reported without changing the console state, and a body that does not
parse SHALL be reported with the position of the error.

A role whose rights resource `Config` does not carry — a role deleted
from the configuration, or one of an extension — SHALL be remembered as
unreadable and asked for once; it SHALL grant nothing, and a command
reading the roles of a user SHALL report how many such roles it holds and
name them without failing. A command naming one role SHALL report that
its rights are not in `Config`.

Every command of this requirement SHALL be offered by name completion.
Users and
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

#### Scenario: Templates of a role
- **WHEN** the user enters `\role ЧтениеЭлектронныхДокументов`
- **THEN** the listing ends with the signatures
  `ДляОбъекта(ПолеОбъекта)`, `ДляРегистра(Регистр, Поле1, …)`,
  `ПоЗначениям()` and `ПоЗначениямРасширенный()`

#### Scenario: Body of one template
- **WHEN** the user enters `\template ЧтениеЭлектронныхДокументов ДляРегистра`
- **THEN** the console prints the signature, what the body is made of,
  and the body of that template

#### Scenario: A role the configuration no longer carries
- **WHEN** `\as` accepts a user holding a role whose `<guid>.0` resource
  is absent from `Config`
- **THEN** the console names that role among those without rights and
  derives the restrictions of the others

#### Scenario: That role asked for by name
- **WHEN** `\role` or `\template` names such a role
- **THEN** the console reports that its rights are not in `Config` and
  changes nothing
