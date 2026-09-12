## ADDED Requirements

### Requirement: Resolve data separator values from session parameters
For every separator field of the snapshot, statement compilation SHALL
determine one value: when the separator's use-flag session parameter is
present with the value `ЛОЖЬ`, the separator is disabled for the
statement; otherwise the value is the session parameter bound as the
separator value, or the session parameter named like the common attribute
when Config binds none. When neither is present, an
`IndependentAndShared` separator SHALL use the empty value of its kind
(`0`, `""`, `ЛОЖЬ`, the empty date) rendered as the dialect's typed
literal, and an `Independent` separator SHALL fail with a `Parameter`
diagnostic at the first source token that reads a table declaring its
column, naming the separator and the expected session parameter. Query
parameters SHALL NOT supply separator values, and preparation (which runs
without values) SHALL NOT fail on them.

#### Scenario: Session value
- **WHEN** `ОбластьДанныхЗначение` is set to `7` in the session parameters
- **THEN** every separated table read by the statement is filtered by
  `"_Fld<N>" = 7`

#### Scenario: Shared default
- **WHEN** no session parameter is set and the separator is
  `IndependentAndShared`
- **THEN** the tables are filtered by the numeric literal `0`

#### Scenario: Disabled separator
- **WHEN** `ОбластьДанныхИспользование` is set to `ЛОЖЬ`
- **THEN** the statement generates no separator predicate

#### Scenario: Independent separator without a value
- **WHEN** no session parameter is set and the separator is `Independent`
- **THEN** compilation fails with a `Parameter` diagnostic that names the
  separator and the session parameter it expects

### Requirement: Filter every separated table by its separator
The compiler SHALL conjoin `<alias>."_Fld<N>" = <value>` for each
separator column declared by the physical table it reads: main tables,
tabular sections, each extension `UNION ALL` branch that declares the
column, change-registration and calculation-kind tables, dereference and
presentation joins, the base reads of slices, balances, and turnovers,
constants, sources inside nested queries and `В (ВЫБРАТЬ …)`, and sources
inside restriction bodies. A source introduced by `ВНУТРЕННЕЕ` or
`ЛЕВОЕ` SHALL carry its predicate in its own `ON`; the first source and a
source introduced by `ПРАВОЕ` SHALL carry it in the `ON` of the next
`ПРАВОЕ` join that null-extends them, or in `WHERE` when none follows;
dereference and presentation joins SHALL carry it in their `ON`. Each
direction of the `ПОЛНОЕ СОЕДИНЕНИЕ` emulation SHALL filter its
null-extended side in `ON` and its preserved side in `WHERE`. Temporary
tables and derived sources SHALL NOT be filtered.
A snapshot without separators, and a table without the column, SHALL
generate SQL byte-identical to a compilation without this requirement.

#### Scenario: Reference filter seeks the primary key
- **WHEN** `ВЫБРАТЬ … ИЗ Справочник.Номенклатура ГДЕ Ссылка = &Ссылка`
  compiles on a separated base
- **THEN** the `WHERE` reads `"_Fld<N>" = <value> AND "_IDRRef" = <id>`

#### Scenario: Left join and dereference
- **WHEN** a catalog is joined with `ЛЕВОЕ СОЕДИНЕНИЕ` and a field of the
  left side is dereferenced
- **THEN** the left side is filtered in `WHERE`, and the joined table and
  the dereference join each carry the predicate in their `ON`

#### Scenario: Full join
- **WHEN** two separated tables are joined with `ПОЛНОЕ СОЕДИНЕНИЕ`
- **THEN** both `LEFT JOIN` directions of the emulation filter the joined
  side in `ON` and the base side in `WHERE`, so no row of another area
  survives as an unmatched row

#### Scenario: Extension branch without the column
- **WHEN** an object's `X1` extension table does not declare the
  separator column
- **THEN** only the base branch of the `UNION ALL` carries the predicate

#### Scenario: Base without separators
- **WHEN** the snapshot has no separator fields
- **THEN** the generated SQL is unchanged
