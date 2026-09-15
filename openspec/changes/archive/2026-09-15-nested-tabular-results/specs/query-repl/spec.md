## ADDED Requirements

### Requirement: Project a tabular section
A projection MAY name a tabular section of a source — `Д.Товары`,
`Д.Товары.(Поле, …)`, or `Д.Товары.*` — which the platform answers as a
nested result inside that column. `Д.Товары` SHALL select the owner
reference, the line number and every attribute of the section; the
parenthesized form SHALL select exactly the named columns; `Д.Товары.*`
SHALL select the same columns as `Д.Товары`. A section used anywhere but a
projection SHALL keep failing with `UnsupportedFeature`.

#### Scenario: Section projected whole
- **WHEN** `ВЫБРАТЬ Д.Номер, Д.Товары ИЗ Документ.Продажа КАК Д` is
  compiled
- **THEN** the query compiles and its nested result carries the rows of the
  section, matching the platform column for column

#### Scenario: Section in a predicate
- **WHEN** a `ГДЕ` names a tabular section
- **THEN** compilation fails, because a section is a table and not a value
