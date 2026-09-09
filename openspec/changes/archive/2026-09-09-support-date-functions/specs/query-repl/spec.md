## ADDED Requirements

### Requirement: Construct date values
The compiler SHALL accept `ДАТАВРЕМЯ`/`DATETIME` with integer year, month, and
day components followed by optional hour, minute, and second components. It
SHALL validate the calendar value and generate a typed date expression for
PostgreSQL and MSSQL. MSSQL generation SHALL apply the configured `_YearOffset`
when the expression participates in a source-backed query and SHALL remove that
offset from projected logical output.

#### Scenario: Date-only constructor
- **WHEN** `ДАТАВРЕМЯ` receives year, month, and day
- **THEN** the omitted time is midnight and generated SQL contains a typed date
  rather than an untyped user-concatenated literal

#### Scenario: Date-time constructor
- **WHEN** `DATETIME` receives all six valid integer components
- **THEN** generated SQL preserves the exact hour, minute, and second

#### Scenario: Invalid constructor
- **WHEN** component count, numeric form, range, calendar date, or offset MSSQL
  year is invalid
- **THEN** compilation returns a positional diagnostic and emits no SQL

### Requirement: Calculate beginning-of-period values
The compiler SHALL accept `НАЧАЛОПЕРИОДА`/`BEGINOFPERIOD` with a date expression
and one of the bilingual minute, hour, day, week, ten-day, month, quarter,
half-year, or year period identifiers. It SHALL generate equivalent native SQL
for PostgreSQL and MSSQL in projections and predicates. Week SHALL begin on
Monday until regional first-weekday metadata becomes part of the compiler
input.

#### Scenario: Nested date constructor
- **WHEN** `НАЧАЛОПЕРИОДА` wraps a `ДАТАВРЕМЯ` expression
- **THEN** the nested typed date is truncated to the requested boundary

#### Scenario: Source field boundary
- **WHEN** a source-backed projection or filter applies `НАЧАЛОПЕРИОДА` to a
  date field
- **THEN** generated SQL evaluates the function in the database and preserves
  MSSQL year-offset semantics

#### Scenario: Virtual-table date argument
- **WHEN** a supported register virtual table receives a constant date-function
  expression as its period argument
- **THEN** the expression is compiled in the physical storage date domain

#### Scenario: Unknown period
- **WHEN** the second argument is absent or is not a supported period identifier
- **THEN** compilation returns a positional diagnostic and emits no SQL
