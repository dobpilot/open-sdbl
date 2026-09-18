## ADDED Requirements

### Requirement: Tuple membership with references of several types
In `(<элементы>) [НЕ] В (<подзапрос>)`, an item or a subquery column that
is a reference of several types SHALL be compared as the RTRef ‖ RRRef
payload, the fixed side widened to it; a composite field item SHALL be
compared member by member with a composite projection of the subquery,
its type-reference member included.

#### Scenario: Recorder and line number
- **WHEN** `(Д.Регистратор, Д.НомерСтроки) В (ВЫБРАТЬ П.Регистратор, П.НомерСтроки ИЗ …)` is compiled
- **THEN** the `EXISTS` compares the recorder payloads and the line
  numbers

#### Scenario: Extra dimensions
- **WHEN** `(Д.СубконтоДт1, Д.СубконтоДт2) В (ВЫБРАТЬ П.СубконтоДт1, П.СубконтоДт2 ИЗ …)` is compiled
- **THEN** each item compares its type column and its payload with the
  projection's members
