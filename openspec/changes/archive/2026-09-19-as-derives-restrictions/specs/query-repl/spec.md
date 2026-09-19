## MODIFIED Requirements

### Requirement: Manage console access restrictions
The console SHALL provide `\restrict <Вид.Объект[.ТабличнаяЧасть]>
<условие>` to store an access restriction for one metadata table or
tabular section, resolving the name against the metadata snapshot at entry
time and replacing an earlier restriction of the same target, `\restrict`
to list stored restrictions with their target and condition, and
`\restrict clear` to forget them all. Every restriction SHALL carry its
origin — typed by the operator, or derived from the roles of the current
user — and the listing SHALL mark the derived ones. A restriction the
operator types SHALL replace a derived one of the same target and become
a typed restriction, and a derived restriction SHALL never replace a
typed one. Before compiling a batch the console
SHALL pass only the restrictions whose target the prepared batch requested,
together with the session parameters, and SHALL print `Restriction`
diagnostics like every other compiler diagnostic. Without stored
restrictions a `РАЗРЕШЕННЫЕ` query SHALL run unfiltered.

#### Scenario: Restricted query
- **WHEN** the user enters `\restrict Справочник.Номенклатура Организация =
  &Орг`, a matching `\session Орг …`, and then
  `ВЫБРАТЬ РАЗРЕШЕННЫЕ … ИЗ Справочник.Номенклатура`
- **THEN** the generated SQL wraps the catalog in the restricted derived
  table

#### Scenario: Restriction for an unread table
- **WHEN** a stored restriction names a table the next query does not read
- **THEN** the query compiles without a diagnostic

#### Scenario: Unknown target
- **WHEN** the user enters `\restrict Справочник.Нет Код = "1"`
- **THEN** the console prints the metadata lookup error and stores nothing

#### Scenario: Typed restriction over a derived one
- **WHEN** the operator types `\restrict` for a target `\as` derived
- **THEN** the typed condition replaces it and the listing no longer
  marks that target as derived

### Requirement: Run allowed queries as a user
`\as <пользователь>` SHALL make that user current and `\as clear` SHALL
forget it; `\as` alone SHALL print the current user. While a user is
current, the console prompt SHALL name it instead of `open-sdbl`, and
the continuation prompt SHALL keep its width; `\as clear` and `\refresh`
SHALL restore `open-sdbl=>`.

`\as <пользователь>` SHALL expand, against the session parameters, the
`Чтение` restrictions every object the user's roles restrict carries, and
SHALL store each expanded condition in the restriction store as a derived
restriction of that object, on one line, leaving the targets a typed
restriction already covers untouched. It SHALL report how many
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
