## MODIFIED Requirements

### Requirement: Verify against the live PostgreSQL catalog read-only
The `open-sdbl-cli` package SHALL provide `open-sdbl metadata postgres` and use
`tokio-postgres` to execute fixed SELECT-only queries in one explicit read-only
`READ COMMITTED` transaction. It SHALL read DBNames, bare-GUID Config
descriptors, SchemaStorage, tables, columns, and indexes and SHALL report
resolved and missing physical objects without extracting, guessing, or printing
1C or PostgreSQL user passwords.

#### Scenario: Resolved information base
- **WHEN** valid connection options identify a PostgreSQL 1C information base
- **THEN** the command prints each resolved GUID, kind, human name, canonical
  physical name, owner table for fields, and live-catalog status

#### Scenario: PostgreSQL authentication
- **WHEN** PostgreSQL requires a password
- **THEN** the CLI obtains it from `PGPASSWORD`, an explicit `PGPASSFILE`, or
  the default `.pgpass` file and does not accept or print it as a command-line
  argument

#### Scenario: Read-only enforcement
- **WHEN** the CLI acquires live metadata
- **THEN** every metadata query executes inside a read-only `READ COMMITTED`
  transaction and no mutating SQL is executed

#### Scenario: No PostgreSQL subprocess
- **WHEN** the CLI connects to PostgreSQL
- **THEN** it uses the asynchronous driver directly and does not require or
  spawn the `psql` executable
