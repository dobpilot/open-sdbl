## ADDED Requirements

### Requirement: A logical expression used as a value
A comparison, `И`/`ИЛИ`/`НЕ`, `ЕСТЬ NULL`, `ПОДОБНО`, `МЕЖДУ`, `ССЫЛКА`
and `В` SHALL be accepted wherever a value is expected, answering a
boolean and answering `NULL` when an operand is `NULL`. In a value
position the expression SHALL be rendered in the boolean value form of the
dialect, so that a dialect without boolean values still receives a value;
in a predicate position it SHALL stay a plain predicate.

#### Scenario: LIKE projected as a column
- **WHEN** `ВЫБРАТЬ Т.Наименование ПОДОБНО "%а%" КАК П ИЗ Справочник.X КАК Т`
  is compiled
- **THEN** the column is projected and its kind is boolean

#### Scenario: A comparison projected on SQL Server
- **WHEN** `ВЫБРАТЬ Т.Поле = 1 КАК П ИЗ Справочник.X КАК Т` is compiled
  for SQL Server
- **THEN** the projection is a value expression, not a bare predicate

#### Scenario: The same comparison in a filter
- **WHEN** `ГДЕ Т.Поле = 1` is compiled
- **THEN** the filter stays a plain predicate
