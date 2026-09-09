## ADDED Requirements

### Requirement: Render SQL for the selected MSSQL dialect level
The MSSQL backend value SHALL carry a dialect level (`Sql2008` or `Sql2012`,
defaulting to `Sql2012`) that every compilation, preparation, and
presentation-lookup path honours. On `Sql2008` generated T-SQL SHALL use
only functions available on SQL Server 2008, emulating newer functions with
equivalent arithmetic, and SHALL produce the same logical values as on
`Sql2012`. Statements that never needed newer functions SHALL be identical
across levels.

#### Scenario: Beginning of period on SQL Server 2008
- **WHEN** `НАЧАЛОПЕРИОДА(Дата, МЕСЯЦ)` is compiled with the `Sql2008` level
- **THEN** generated SQL uses `DATEADD`/`DATEDIFF` from a `datetime2` base
  and contains no `DATETIME2FROMPARTS`

#### Scenario: Level parity
- **WHEN** a query without `НАЧАЛОПЕРИОДА` is compiled on both levels
- **THEN** the generated SQL is identical

#### Scenario: Default level
- **WHEN** an application constructs `MsSqlBackend::new(year_offset)` without
  choosing a level
- **THEN** the backend reports `Sql2012` and generates the same SQL as before
  levels existed

### Requirement: Emit PostgreSQL SQL portable to 9.0
Generated PostgreSQL SQL SHALL avoid constructs introduced after PostgreSQL
9.0 so that one stateless backend serves every supported server.

#### Scenario: Balance anchor aggregate
- **WHEN** an accumulation-register balance query is compiled
- **THEN** the anchor period uses `MAX(CASE WHEN … END)` rather than
  `FILTER (WHERE …)`
