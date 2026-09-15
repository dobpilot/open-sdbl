## ADDED Requirements

### Requirement: Compare a composite column of a tabular section
The `EXISTS` of a tabular-section predicate MAY compare a column stored as
a reference pair. Such a column SHALL be compared by its `RTRef ‖ RRRef`
payload, and the other side SHALL be widened to a payload as it is in any
other comparison of a composite reference. A column that is composite in
another way SHALL keep its diagnostic.

#### Scenario: Section column holding any reference
- **WHEN** a predicate compares `Задача.ЗадачаИсполнителя.Предметы.Предмет`
  with a catalog reference
- **THEN** the `EXISTS` compares the payload of the section column with
  the widened reference, and the owner answers once

#### Scenario: Section column that is not a reference pair
- **WHEN** the compared column spreads over members that are not a
  reference pair
- **THEN** compilation fails, naming the column
