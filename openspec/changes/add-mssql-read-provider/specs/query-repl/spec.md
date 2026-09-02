## ADDED Requirements

### Requirement: Compile bounded SDBL to Microsoft SQL Server
The core library SHALL expose MSSQL compile and prepare APIs that reuse the
existing bounded SDBL parser, metadata resolution, and presentation-plan
protocol while emitting native T-SQL. Existing PostgreSQL APIs and generated
PostgreSQL SQL SHALL remain compatible.

#### Scenario: MSSQL projection and limit
- **WHEN** an MSSQL query selects logical 1C fields with `ПЕРВЫЕ`/`TOP`
- **THEN** generated T-SQL quotes physical identifiers, converts supported
  text-rendered values, and applies `TOP` in SQL Server

#### Scenario: MSSQL rowversion projection
- **WHEN** a selected logical field is backed by an MSSQL `timestamp` or
  `rowversion` column
- **THEN** generated T-SQL projects the physical column without `CAST` or
  `CONVERT`, and the CLI renders the received binary value as hexadecimal text

#### Scenario: Binary literal comparison
- **WHEN** a filter compares a binary or rowversion field with a validated
  `0x` hexadecimal literal
- **THEN** MSSQL T-SQL contains a native varbinary literal and PostgreSQL SQL
  contains an equivalent `bytea` literal without treating the value as text

#### Scenario: Unicode and binary values
- **WHEN** an MSSQL query contains Cyrillic strings or compares reference type
  discriminators
- **THEN** generated T-SQL uses Unicode string literals and native varbinary
  literals without PostgreSQL casts

#### Scenario: Dialect isolation
- **WHEN** the same supported SDBL is compiled for PostgreSQL and MSSQL
- **THEN** each output uses its native limit, cast, literal, aggregate, and
  virtual-table syntax without textual post-processing

### Requirement: Provide a read-only MSSQL console
The CLI SHALL provide `open-sdbl console mssql` and `open-sdbl metadata mssql`
using SQL Server authentication, TDS over direct TCP or the existing SOCKS5
transport, TLS certificate validation by default, and an optional explicit
server-certificate trust flag. The password SHALL be read from
`MSSQL_PASSWORD`. The console SHALL request read-only application intent and
execute only fixed metadata SELECTs or SQL generated from the bounded SELECT
compiler.

#### Scenario: MSSQL connection defaults
- **WHEN** a user supplies host, database, user, and `MSSQL_PASSWORD`
- **THEN** the CLI connects to port 1433 with TLS validation and read-only
  application intent

#### Scenario: Explicit certificate trust
- **WHEN** the server uses an untrusted certificate and the user supplies
  `--trust-server-certificate`
- **THEN** the CLI opts out of certificate validation for that connection and
  documents the security tradeoff

#### Scenario: Missing password
- **WHEN** SQL authentication is requested without `MSSQL_PASSWORD`
- **THEN** the CLI fails before opening a connection and does not print a
  password value

#### Scenario: Query execution
- **WHEN** the user enters a supported semicolon-terminated SDBL SELECT
- **THEN** the console prints native T-SQL, execution time, textual columns,
  rows, and row count and remains available after recoverable errors

#### Scenario: Defense in depth
- **WHEN** the MSSQL provider is deployed
- **THEN** documentation requires a SQL login whose effective permissions are
  limited to SELECT because read-only application intent is not authorization
