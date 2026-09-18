## ADDED Requirements

### Requirement: Automatic ordering is accepted
`АВТОУПОРЯДОЧИВАНИЕ` after the keys of `УПОРЯДОЧИТЬ ПО`, or where the
clause would stand, SHALL be accepted and SHALL not change the SQL.

#### Scenario: After the keys
- **WHEN** `… УПОРЯДОЧИТЬ ПО Код АВТОУПОРЯДОЧИВАНИЕ` is compiled
- **THEN** the SQL orders by the code alone

### Requirement: Record auto number
`АВТОНОМЕРЗАПИСИ()` SHALL compile to `ROW_NUMBER() OVER (ORDER BY (SELECT
NULL))` of kind number in any statement; an argument SHALL be a `Syntax`
diagnostic naming zero arguments.

#### Scenario: Numbered projection
- **WHEN** `ВЫБРАТЬ АВТОНОМЕРЗАПИСИ() КАК Номер, Code ИЗ …` is compiled
- **THEN** the first column is the row number
