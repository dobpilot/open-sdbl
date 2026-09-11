## ADDED Requirements

### Requirement: Manage console session parameters
The console SHALL provide `\session <Имя> [=] <литерал>` to store a session
parameter from the same SDBL literals `\set` accepts, `\session` to list
stored session parameters with their literal text and value kind, and
`\session clear` to forget them all; `\session` with a malformed argument
SHALL print the command syntax. Every query SHALL be compiled with all
stored session parameters, so an unreferenced session parameter is never
an error, and a `\set` parameter referenced by the statement SHALL take
precedence over a session parameter of the same name.

#### Scenario: Session parameter used by a query
- **WHEN** the user enters `\session ТекущийПользователь = "Иванов"` and
  then a query referencing `&ТекущийПользователь`
- **THEN** the query compiles with the stored string

#### Scenario: Unreferenced session parameter
- **WHEN** a stored session parameter is not referenced by the next query
- **THEN** the query compiles without a diagnostic

### Requirement: Manage console access restrictions
The console SHALL provide `\restrict <Вид.Объект[.ТабличнаяЧасть]>
<условие>` to store an access restriction for one metadata table or
tabular section, resolving the name against the metadata snapshot at entry
time and replacing an earlier restriction of the same target, `\restrict`
to list stored restrictions with their target and condition, and
`\restrict clear` to forget them all. Before compiling a batch the console
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
