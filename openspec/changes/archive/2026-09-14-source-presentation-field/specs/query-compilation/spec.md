## ADDED Requirements

### Requirement: Presentation of a source
`Источник.Представление` SHALL present the reference of that source,
deferred to the application exactly as `ПРЕДСТАВЛЕНИЕССЫЛКИ` of its
reference field is. A source whose rows carry no reference SHALL be
refused with a diagnostic naming it.

#### Scenario: Presentation of a catalog source
- **WHEN** `ВЫБРАТЬ Т.Представление ИЗ Справочник.X КАК Т` is compiled
- **THEN** the column is requested as a presentation of that catalog's
  reference
