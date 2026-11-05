## ADDED Requirements

### Requirement: List the parameters of a prepared query
`Prepared::parameter_names` SHALL return the distinct names of the `&Имя`
parameters the query text references, in the order of their first
occurrence, compared case-insensitively and spelled as at their first
occurrence. Names inside string literals and comments SHALL NOT be listed.
Session parameters a restriction expands into the query SHALL NOT be
listed. The list SHALL be computed without I/O and SHALL be the same for
both backends.

#### Scenario: Order of first occurrence
- **WHEN** `ВЫБРАТЬ 1 ГДЕ &Б > 0 И &а > 0 И &б < 5` is prepared
- **THEN** the names are `Б`, `а`

#### Scenario: Parameter-like text in a literal
- **WHEN** a query compares a field with the string `"&Имя"` and references
  no parameter
- **THEN** the list is empty

#### Scenario: Temporary-table batch
- **WHEN** a batch references `&Дата` in its first statement and `&Склад`
  in its second
- **THEN** the names are `Дата`, `Склад`
