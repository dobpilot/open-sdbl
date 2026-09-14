## ADDED Requirements

### Requirement: Presenting an expression
`ПРЕДСТАВЛЕНИЕ` SHALL accept any expression. A value that is not a
reference SHALL be presented as itself. A reference expression SHALL be
refused with a diagnostic saying that only a reference field can be
presented, because the presentation of a reference comes from the
application and is requested for a field.

#### Scenario: Presenting a concatenation
- **WHEN** `ВЫБРАТЬ ПРЕДСТАВЛЕНИЕ(Т.Код + "!") ИЗ Справочник.X КАК Т` is
  compiled
- **THEN** the column carries the concatenated string

#### Scenario: Presenting a reference expression
- **WHEN** the argument is an expression whose value is a reference
- **THEN** the query is refused, naming the reason
