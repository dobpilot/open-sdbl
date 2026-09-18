## ADDED Requirements

### Requirement: Unaliased fields are labelled as written
A projected field without `КАК` SHALL carry the label of its last path
segment as the text spells it — `Ссылка` for `Т.Ссылка`, `Ref` for
`Т.Ref`, `Регистратор` for `Д.Регистратор` — with the member suffixes of
a compound field appended; a nested query or temporary table exposes the
column under that name.

#### Scenario: Temporary table read by the written name
- **WHEN** `ВЫБРАТЬ Д.Регистратор ПОМЕСТИТЬ ВТ ИЗ … КАК Д; ВЫБРАТЬ ВТ.Регистратор ИЗ ВТ КАК ВТ;`
  is compiled
- **THEN** the table's column is `Регистратор` and the second statement
  reads it
