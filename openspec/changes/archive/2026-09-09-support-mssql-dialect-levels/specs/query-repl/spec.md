## ADDED Requirements

### Requirement: Detect the MSSQL dialect level at connection
When connecting to Microsoft SQL Server the CLI SHALL read
`SERVERPROPERTY('ProductVersion')`, map major versions below 11 to the
`Sql2008` level and 11 or above to `Sql2012`, print the chosen level and the
server version, and use that level for every compiled query. The
`--mssql-dialect 2008|2012` option SHALL override detection and SHALL be
rejected for other providers or values.

#### Scenario: SQL Server 2008 R2
- **WHEN** the server reports product version `10.50.6000.34`
- **THEN** the console prints the `2008` level and `НАЧАЛОПЕРИОДА` queries
  execute without error 195

#### Scenario: Explicit override
- **WHEN** the user passes `--mssql-dialect 2008` against SQL Server 2019
- **THEN** the console compiles with the `Sql2008` level and the query results
  equal those of the default level
