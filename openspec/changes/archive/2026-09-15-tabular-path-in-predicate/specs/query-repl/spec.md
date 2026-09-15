## ADDED Requirements

### Requirement: Test a tabular-section column in a predicate
A comparison MAY name a column of a tabular section of one of the
statement's sources as `<источник>.<Состав>.<Поле>`. It SHALL compile into
an `EXISTS` over the section correlated with the owner row, because that
is what the platform answers: a row of the owner appears once when any of
its section rows satisfies the comparison, never once per matching row.

The path SHALL remain refused outside a comparison — in a projection, a
grouping key or an ordering key — where existence is not its meaning.

#### Scenario: Owner with several matching rows
- **WHEN** `ВЫБРАТЬ Д.Номер ИЗ Документ.Продажа КАК Д ГДЕ
  Д.Товары.Количество > 2` is compiled over a document whose section has
  two rows above two
- **THEN** that document answers once, as the platform answers, and
  `КОЛИЧЕСТВО(*)` counts it once

#### Scenario: Correlated with an outer source
- **WHEN** a subquery selects from a task and compares
  `Задача.ЗадачаИсполнителя.Предметы.Предмет` with a column of the outer
  query
- **THEN** the `EXISTS` carries both correlations

#### Scenario: Path in a projection
- **WHEN** a projection names `Д.Товары.Количество`
- **THEN** compilation fails, because a section column is not a value of
  the owner row
