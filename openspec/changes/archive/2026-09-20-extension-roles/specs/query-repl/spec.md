## MODIFIED Requirements

### Requirement: Run allowed queries as a user
`\as <пользователь>` SHALL make that user current and `\as clear` SHALL
forget it; `\as` alone SHALL print the current user. While a user is
current, the console prompt SHALL name it instead of `open-sdbl`, and
the continuation prompt SHALL keep its width; `\as clear` and `\refresh`
SHALL restore `open-sdbl=>`.

The rights of a role the configuration does not carry SHALL be looked for
in the configuration extensions: the console SHALL read their resource
index once, take the rights resource `<guid>.0` of the role from the
store, and name the role by its descriptor there. Only a role neither the
configuration nor an extension carries SHALL be reported as granting
nothing.

`\as <пользователь>` SHALL read the session parameters the base carries
for its restriction templates — the values the information register
`ПараметрыОграниченияДоступа` stores, the element of
`Справочник.Пользователи` whose `ИдентификаторПользователяИБ` is the
identifier of that user as `ТекущийПользователь`, and the empty reference
of `Справочник.ВнешниеПользователи` for the current external user, each
when the configuration has it — SHALL store only the ones no `\session`
already holds, so what the operator typed always wins, and SHALL name
those it stored. A configuration without those tables, and a user without
a row in the catalog, SHALL be no error.

`\as <пользователь>` SHALL then expand, against the session parameters,
the `Чтение` restrictions every object the user's roles restrict carries,
and SHALL store each expanded condition in the restriction store as a
derived restriction of that object, on one line, leaving the targets a
typed restriction already covers untouched. It SHALL report how many
restrictions it derived and, for the objects whose expansion failed, each
distinct message with the number of objects it applies to, without
failing the command. `\as clear` and `\refresh` SHALL forget the derived
restrictions, and a `\session` command that stores or clears a value
SHALL derive them again while a user is current, reporting as `\as` does,
so a stored condition is never older than the parameters it was expanded
with.

With a current user,
every target a `РАЗРЕШЕННЫЕ` batch requests that no restriction covers
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

#### Scenario: Restrictions derived into the store
- **WHEN** `\as` accepts a user whose roles restrict reading of a catalog
  and the session parameters the templates read have values
- **THEN** `\restrict` lists that catalog with the expanded condition,
  marked as derived, and `\as clear` forgets it

#### Scenario: Restriction that cannot be expanded
- **WHEN** a template of one object reads a session parameter without a
  value
- **THEN** `\as` reports that message with the number of objects it
  applies to, stores the restrictions it could expand, and stays the
  current user

#### Scenario: Parameters read from the base
- **WHEN** `\as` accepts a user on the УНФ demo base
- **THEN** it names the parameters it read from
  `ПараметрыОграниченияДоступа` and derives the restrictions with them

#### Scenario: A typed parameter is kept
- **WHEN** `\session ВерсииШаблоновОграниченияДоступа …` was entered
  before `\as`
- **THEN** that value stays and `\as` does not name it among those read

#### Scenario: The current user of the base
- **WHEN** `\as ХохловМЗ` is accepted on a base whose
  `Справочник.Пользователи` carries that information-base user
- **THEN** `ТекущийПользователь` holds the reference of that element and
  a restriction comparing a row with it compiles

#### Scenario: A role of an extension
- **WHEN** `\as` accepts a user holding `_ДемоБазовыеПрава`, a role of
  the extension `_ДемоРасширение`
- **THEN** the console names that role and takes the rights its
  `<guid>.0` resource carries in the extension store
