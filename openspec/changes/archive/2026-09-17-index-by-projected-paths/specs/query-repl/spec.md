## ADDED Requirements

### Requirement: Index fields by name and by projected path
An index field of `ИНДЕКСИРОВАТЬ ПО` SHALL be accepted when its last
segment equals a column's emitted label or the alias the text gave that
column, or — for a qualified field — when the first branch projects
exactly that field path under any alias.

#### Scenario: Qualified field projected under another alias
- **WHEN** `ВЫБРАТЬ Т.Code КАК Код ПОМЕСТИТЬ ВТ ИЗ … КАК Т ИНДЕКСИРОВАТЬ ПО Т.Code`
  is compiled
- **THEN** the statement compiles

#### Scenario: Unprojected qualified field
- **WHEN** `ИНДЕКСИРОВАТЬ ПО Т.Date` names a field the branch does not
  project
- **THEN** the diagnostic is `TemporaryTable`
