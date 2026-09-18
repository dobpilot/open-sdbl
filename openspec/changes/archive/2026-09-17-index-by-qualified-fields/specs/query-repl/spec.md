## ADDED Requirements

### Requirement: Qualified index fields
`ИНДЕКСИРОВАТЬ ПО` SHALL accept a field written as a path
(`Псевдоним.Поле`); the last segment SHALL name the selection-list
label, and a label absent from the selection list SHALL remain a
`TemporaryTable` diagnostic.

#### Scenario: Index field qualified by the source alias
- **WHEN** `ВЫБРАТЬ Т.Код, Т.Наименование ПОМЕСТИТЬ ВТ ИЗ Справочник.Номенклатура КАК Т ИНДЕКСИРОВАТЬ ПО Т.Код, Наименование;`
  is compiled
- **THEN** the statement compiles as with `ИНДЕКСИРОВАТЬ ПО Код, Наименование`

#### Scenario: Qualified field outside the selection list
- **WHEN** `ИНДЕКСИРОВАТЬ ПО Т.Артикул` names a label not projected
- **THEN** the diagnostic is `TemporaryTable`
