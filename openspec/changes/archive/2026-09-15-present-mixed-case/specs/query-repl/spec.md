## ADDED Requirements

### Requirement: Present a ВЫБОР whose branches differ in type
`ПРЕДСТАВЛЕНИЕ` of a `ВЫБОР` SHALL present each branch on its own and
answer one string column: a string branch answers itself, a reference
branch answers the presentation the application's plan builds, and a
`NULL` branch answers `NULL`. The branches SHALL NOT be required to share
one kind, because the platform answers such a `ВЫБОР` branch by branch.

A branch that can only be answered by the deferred protocol — a reference
expression that is not a field — SHALL fail with `UnsupportedFeature`,
since a column is either deferred as a whole or built from a plan.

#### Scenario: String beside a reference
- **WHEN** `ПРЕДСТАВЛЕНИЕ(ВЫБОР КОГДА Т.Цена > 5 ТОГДА "нет" ИНАЧЕ
  Т.Клиент КОНЕЦ)` is compiled
- **THEN** the string rows answer the string and the reference rows answer
  the presentation of the reference, as the platform answers

#### Scenario: Composite field as the subject and a branch
- **WHEN** the `ВЫБОР` chooses by a composite reference field and falls
  back to that field
- **THEN** the presentation compiles, with the composite branch presented
  through its targets
