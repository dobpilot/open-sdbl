## ADDED Requirements

### Requirement: Condition without a source
A statement without a source SHALL accept `ГДЕ` and render it as a
`WHERE` clause with no `FROM`, which is what the platform answers: a false
condition yields no row and a true one yields the single row of the
projection.

#### Scenario: Constant row filtered away
- **WHEN** `ВЫБРАТЬ 1 КАК Т ГДЕ ЛОЖЬ` is compiled and executed
- **THEN** no row is answered

### Requirement: Value type of a chart of characteristic types
`ТипЗначения` / `ValueType` SHALL name the `Type` column of a chart of
characteristic types.

#### Scenario: Projecting the value type
- **WHEN** `ВЫБРАТЬ П.ТипЗначения ИЗ ПланВидовХарактеристик.X КАК П` is
  compiled
- **THEN** the column reads the chart's `Type` column
