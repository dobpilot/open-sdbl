## MODIFIED Requirements

### Requirement: Presenting an expression
`ПРЕДСТАВЛЕНИЕ` SHALL accept any expression. A value that is not a
reference SHALL be presented as itself. A value that is a reference SHALL
be carried as a deferred presentation column, the way a universal
reference field is, so that the application resolves it.

#### Scenario: Presenting a concatenation
- **WHEN** `ВЫБРАТЬ ПРЕДСТАВЛЕНИЕ(Т.Код + "!") ИЗ Справочник.X КАК Т` is
  compiled
- **THEN** the column carries the concatenated string

#### Scenario: Presenting a reference expression
- **WHEN** the argument is an expression whose value is a reference
- **THEN** the column carries the reference itself and is reported as a
  deferred presentation
