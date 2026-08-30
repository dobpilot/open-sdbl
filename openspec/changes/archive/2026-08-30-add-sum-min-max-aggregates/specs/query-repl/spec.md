## ADDED Requirements

### Requirement: Compile SUM, MIN, and MAX projections
The compiler SHALL accept bilingual `SUM`/`СУММА`, `MIN`/`МИНИМУМ`, and
`MAX`/`МАКСИМУМ` with one resolved field argument and compile the corresponding
PostgreSQL aggregate cast to text. `COUNT(DISTINCT field)` and its Russian form
SHALL remain supported. Wildcard and DISTINCT SHALL be accepted only for COUNT.
All aggregates SHALL share the existing compound-field, projection-mixing, and
transposed FULL JOIN safety checks.

#### Scenario: Numeric sum
- **WHEN** a query selects `СУММА(<numeric-field>)`
- **THEN** generated PostgreSQL applies SUM to the resolved physical column

#### Scenario: Minimum and maximum
- **WHEN** a query selects `MIN(field)` and `МАКСИМУМ(field)`
- **THEN** generated PostgreSQL returns both aggregate values as text

#### Scenario: Distinct count remains supported
- **WHEN** a query selects `COUNT(DISTINCT field)`
- **THEN** generated PostgreSQL retains DISTINCT inside COUNT

#### Scenario: Invalid SUM wildcard
- **WHEN** a query uses `SUM(*)`
- **THEN** compilation reports that wildcard is supported only by COUNT
