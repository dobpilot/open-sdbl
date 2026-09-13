## MODIFIED Requirements

### Requirement: Emit native-typed projections
Generated SQL SHALL project physical columns, scalar expressions, and
aggregates in their native database types without converting them to text.
The only conversions permitted are the MSSQL `_YearOffset` correction that
returns logical dates for date columns and a text cast for PostgreSQL
`mchar`/`mvarchar` values of the 1C extension, whose binary wire format is
undocumented. That cast SHALL apply both to a projected column of such a
type and to a projected computed expression of kind `String`, whose
operands may carry the extension type, in nested statements as well as in
the outer statement; an expression already rendered with a trailing text
cast SHALL NOT be cast again. Presentation functions MAY still convert
their arguments to text because their result is a string.

#### Scenario: Native reference projection
- **WHEN** a query projects a catalog `Ссылка`
- **THEN** generated SQL selects the physical reference column without a hex
  or text conversion

#### Scenario: MSSQL date with year offset
- **WHEN** a date column is projected for MSSQL with a non-zero year offset
- **THEN** generated SQL wraps it in `DATEADD(year, -offset, …)` and emits no
  `CONVERT`

#### Scenario: PostgreSQL 1C string type
- **WHEN** a `mvarchar` column is projected for PostgreSQL
- **THEN** generated SQL casts it to `text`

#### Scenario: PostgreSQL character expression
- **WHEN** a query projects `ЕСТЬNULL(Т.Наименование, "нет")`,
  `ВЫБОР … ТОГДА Т.Наименование … КОНЕЦ`, or `МАКСИМУМ(Т.Наименование)`
  over such a column
- **THEN** generated SQL casts that expression to `text`, and the driver
  decodes the value
