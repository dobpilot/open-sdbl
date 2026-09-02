## ADDED Requirements

### Requirement: Acquire authoritative 1C metadata from Microsoft SQL Server
The application SHALL read DBNames from `Params`, GUID-named part-zero Config
resources from `Config`, the current schema from `SchemaStorage`, and live
tables, columns, and ordered index keys from the SQL Server `dbo` catalog using
fixed SELECT-only queries supplied by the core library. Returned resources
SHALL be decoded and resolved by the same deterministic core APIs as PostgreSQL.

#### Scenario: SQL Server DBNames and Config
- **WHEN** `metadata mssql` connects to a supported 1C SQL Server database
- **THEN** it reads binary DBNames and Config resources without converting or
  truncating their bytes

#### Scenario: SQL Server live catalog
- **WHEN** the database contains physical 1C tables and indexes in `dbo`
- **THEN** metadata resolution receives their case-preserved names, normalized
  type declarations, uniqueness flags, and key columns in ordinal order

#### Scenario: Acquisition safety
- **WHEN** all MSSQL metadata query definitions are inspected
- **THEN** every statement is SELECT-only and scoped to the connected database

### Requirement: Read the SQL Server 1C year offset
The MSSQL application provider SHALL read `dbo._YearOffset.Offset`, accept the
1C values 0 and 2000, and pass the value to MSSQL query compilation.

#### Scenario: Offset 2000 infobase
- **WHEN** `_YearOffset.Offset` is 2000
- **THEN** projected physical datetime values are shifted back by 2000 years
  and logical date literals used for database filtering are shifted forward by
  2000 years
