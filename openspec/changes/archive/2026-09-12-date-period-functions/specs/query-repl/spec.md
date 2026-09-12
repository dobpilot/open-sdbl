## MODIFIED Requirements

### Requirement: Calculate beginning-of-period values
The compiler SHALL accept `НАЧАЛОПЕРИОДА`/`BEGINOFPERIOD` with a date expression
and one of the bilingual minute, hour, day, week, ten-day, month, quarter,
half-year, or year period identifiers. It SHALL generate equivalent native SQL
for PostgreSQL and MSSQL in projections and predicates. Week SHALL begin on
Monday until regional first-weekday metadata becomes part of the compiler
input. A known period the function does not accept (`СЕКУНДА`) SHALL be a
`Syntax` diagnostic; an unknown period name SHALL be an `UnsupportedFeature`
diagnostic. Virtual-table period arguments SHALL accept `ДАТАВРЕМЯ`, a date
parameter, and `НАЧАЛОПЕРИОДА`, `КОНЕЦПЕРИОДА`, or `ДОБАВИТЬКДАТЕ` nested
over those (the count of `ДОБАВИТЬКДАТЕ` being a numeric literal or
parameter), compiled in the physical storage date domain.

#### Scenario: Nested date constructor
- **WHEN** `НАЧАЛОПЕРИОДА` wraps a `ДАТАВРЕМЯ` expression
- **THEN** the nested typed date is truncated to the requested boundary

#### Scenario: Source field boundary
- **WHEN** a source-backed projection or filter applies `НАЧАЛОПЕРИОДА` to a
  date field
- **THEN** generated SQL evaluates the function in the database and preserves
  MSSQL year-offset semantics

#### Scenario: Virtual-table date argument
- **WHEN** `Обороты(НАЧАЛОПЕРИОДА(&П, МЕСЯЦ), КОНЕЦПЕРИОДА(&П, МЕСЯЦ))`
  is compiled with a date value for `&П`
- **THEN** both bounds render the inlined date in the storage domain,
  truncated to the month and extended to its last second

#### Scenario: Unknown period
- **WHEN** the second argument is absent or is not a supported period identifier
- **THEN** compilation returns a positional diagnostic and emits no SQL

#### Scenario: Second period
- **WHEN** `НАЧАЛОПЕРИОДА(Дата, СЕКУНДА)` is compiled
- **THEN** compilation fails with a `Syntax` diagnostic at the period token

## ADDED Requirements

### Requirement: Calculate end-of-period values
The compiler SHALL accept `КОНЕЦПЕРИОДА`/`ENDOFPERIOD` with a date
expression and the nine periods of `НАЧАЛОПЕРИОДА` and SHALL render the
last second of the period on PostgreSQL and both MSSQL dialect levels:
the beginning of the next period minus one second, where a ten-day period
ends on the 10th, the 20th, or the last day of the month and a week ends
on Sunday. The result kind SHALL be date.

#### Scenario: End of month
- **WHEN** `КОНЕЦПЕРИОДА(ДАТАВРЕМЯ(2020, 2, 10), МЕСЯЦ)` is compiled
- **THEN** the SQL evaluates to `2020-02-29 23:59:59` on both providers

#### Scenario: End of ten-day period
- **WHEN** `КОНЕЦПЕРИОДА(Дата, ДЕКАДА)` is applied to `2020-06-15` and to
  `2020-02-29`
- **THEN** the results are `2020-06-20 23:59:59` and `2020-02-29 23:59:59`

### Requirement: Shift dates by periods
The compiler SHALL accept `ДОБАВИТЬКДАТЕ`/`DATEADD` with a date
expression, one of the bilingual second, minute, hour, day, week, ten-day,
month, quarter, half-year, or year periods, and a count that is a numeric
expression, field, or parameter. Month-based shifts SHALL clamp to the
last day of the target month. A fractional count SHALL be rounded half
away from zero for second, minute, hour, day, week, and month, and
truncated toward zero for ten-day, quarter, half-year, and year, matching
the platform. The result kind SHALL be date and the count SHALL be a
number-kind, parameter, or unknown-kind expression, otherwise compilation
fails with a `Syntax` diagnostic.

#### Scenario: Month-end clamping
- **WHEN** `ДОБАВИТЬКДАТЕ(ДАТАВРЕМЯ(2020, 1, 31, 23, 59, 59), МЕСЯЦ, 1)`
  is compiled
- **THEN** the SQL evaluates to `2020-02-29 23:59:59` on both providers

#### Scenario: Fractional count
- **WHEN** `ДОБАВИТЬКДАТЕ(Дата, ДЕНЬ, 1.5)` and `ДОБАВИТЬКДАТЕ(Дата, ГОД,
  1.5)` are compiled
- **THEN** the first adds two days and the second adds one year

#### Scenario: Count from a field
- **WHEN** `ДОБАВИТЬКДАТЕ(Т.Дата, ДЕНЬ, Т.Количество)` is compiled for a
  numeric field
- **THEN** generated SQL adds the field's rounded value in days

### Requirement: Compute date differences
The compiler SHALL accept `РАЗНОСТЬДАТ`/`DATEDIFF` with two date
expressions and one of the bilingual second, minute, hour, day, month,
quarter, or year units and SHALL return the number of unit boundaries
crossed from the first date to the second (negative when the second date
is earlier), as SQL Server's `DATEDIFF` counts them; `НЕДЕЛЯ`, `ДЕКАДА`,
and `ПОЛУГОДИЕ` SHALL be `Syntax` diagnostics. Seconds, minutes, and hours
SHALL not overflow 32 bits over the full 1C date range on either provider.
On MSSQL with a non-zero year offset both operands SHALL be shifted to
the logical date before the difference is taken. The result kind SHALL be
number.

#### Scenario: Boundary counting
- **WHEN** `РАЗНОСТЬДАТ(ДАТАВРЕМЯ(2020, 12, 31, 23, 59, 59), ДАТАВРЕМЯ(2021, 1, 1), ДЕНЬ)`
  is compiled
- **THEN** the SQL evaluates to `1` on both providers, and `ГОД` gives `1`

#### Scenario: Seconds over the full range
- **WHEN** `РАЗНОСТЬДАТ(ДАТАВРЕМЯ(1, 1, 1), ДАТАВРЕМЯ(2021, 3, 15, 10, 30, 30), СЕКУНДА)`
  is compiled
- **THEN** the SQL evaluates to `63751401030` on both providers

#### Scenario: Unsupported unit
- **WHEN** `РАЗНОСТЬДАТ(А, Б, НЕДЕЛЯ)` is compiled
- **THEN** compilation fails with a `Syntax` diagnostic at the unit token
