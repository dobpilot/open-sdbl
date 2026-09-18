## MODIFIED Requirements

### Requirement: Alternatives of different types
A projected `ВЫБОР` or `ЕСТЬNULL` whose alternatives differ in type SHALL
be rendered as the members of a composite value, the way the platform
stores one: the `_TYPE` discriminator naming the type of each row, one
column per type present among the alternatives, and the reference payload
when a branch carries a reference. Each branch SHALL write its value into
its own member and the zero of the type into the others. Each member
SHALL carry the output label of the projection with the suffix a projected
composite field uses. Compared with a value, such a `ВЫБОР` SHALL render
an alternative of another kind than the value as `NULL`, so that the
comparison never holds for it, as the platform answers; compared with
`NULL` or a parameter without a value, the alternatives SHALL be
measured against the first typed one. `<>` against such an alternative
answers `NULL` rather than the platform's true.

#### Scenario: String and reference alternatives
- **WHEN** `ВЫБОР КОГДА … ТОГДА "дорого" ИНАЧЕ Т.Клиент КОНЕЦ КАК Смесь`
  is projected
- **THEN** the result carries `Смесь` with the reference payload,
  `Смесь_S` with the string and `Смесь_TYPE` with the discriminator

#### Scenario: Alternatives of two primitive types
- **WHEN** the alternatives are a number and a string
- **THEN** only the number, string and discriminator members are projected

#### Scenario: Reference and boolean alternatives compared
- **WHEN** `ГДЕ ВЫБОР КОГДА Д.Проведен ТОГДА Д.Организация ИНАЧЕ ЛОЖЬ КОНЕЦ = &Организация`
  is compiled
- **THEN** the `CASE` keeps the reference alternative and renders `ЛОЖЬ`
  as `NULL`, compared with the parameter
