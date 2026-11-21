## MODIFIED Requirements

### Requirement: Serve the PostgreSQL protocol for SDBL
The `open-sdbl-pgwire` package SHALL accept client connections on a
caller-provided listener, speak PostgreSQL frontend/backend protocol 3.0
to them, and treat every query text as SDBL except the fixed whitelist of
client chatter and catalog SQL routed to the virtual system catalog. It
SHALL serve 1C bases stored in PostgreSQL and in Microsoft SQL Server
through the same client protocol.

#### Scenario: psql runs an SDBL query on a PostgreSQL base
- **WHEN** `psql` connects to the gateway serving a PostgreSQL 1C base and
  sends `ВЫБРАТЬ Наименование ИЗ Справочник.Номенклатура`
- **THEN** it receives a `RowDescription` with one `text` column named
  `Наименование`, the rows of the catalog, and `CommandComplete SELECT n`

#### Scenario: The same query on an MSSQL base
- **WHEN** the gateway serves an MSSQL 1C base and receives the same text
- **THEN** the client receives the same column description and rows in the
  PostgreSQL protocol

#### Scenario: Catalog SQL is answered by the virtual catalog
- **WHEN** a client sends `SELECT nspname FROM pg_catalog.pg_namespace`
- **THEN** it receives the schemas of the virtual catalog instead of an
  SDBL diagnostic

### Requirement: Never forward client text to the database
The gateway SHALL send to the database only the SELECT statements the
compiler generates and the fixed metadata and rights acquisition
statements. It SHALL NOT forward client text in any mode.

#### Scenario: SQL text is not passed through
- **WHEN** a client sends `SELECT * FROM _Reference42`
- **THEN** the gateway answers an `ErrorResponse` with the SDBL diagnostic
  and sends nothing to the database

#### Scenario: Catalog SQL never reaches the database
- **WHEN** a client sends any query the virtual catalog evaluates
- **THEN** the database receives no statement for it

## ADDED Requirements

### Requirement: Identify the configured backing database
The virtual `pg_database`, `current_database()`, `current_catalog` and
information-schema catalog names SHALL use the gateway's configured
backing database (CLI `--database`) for both providers. Client startup
database names or defaults SHALL NOT rename or switch that database.

#### Scenario: Client defaults its database to the login
- **WHEN** a client connects with database `EDIEvents` or its login name
  while the gateway serves `--database unf`
- **THEN** all virtual database/catalog names report `unf`

### Requirement: Answer recorded DataGrip bootstrap probes
The gateway SHALL answer the recorded DataGrip SSL-status and rounded
server-start-epoch probes locally, with boolean and numeric result types.
SSL status SHALL describe the actual client transport. Start time SHALL
belong to the gateway instance, remain constant across connections and
metadata refresh, and never require a backend query. Unsupported variants
SHALL retain existing routing rather than receive a fabricated response.

#### Scenario: DataGrip checks SSL
- **WHEN** it executes `select ssl from pg_stat_ssl where pid = pg_backend_pid()`
- **THEN** it receives the connection's TLS status

#### Scenario: DataGrip checks server restart
- **WHEN** it executes `select round(extract(epoch from pg_postmaster_start_time() at time zone 'UTC')) as startup_time`
- **THEN** it receives the gateway's stable rounded Unix startup epoch

### Requirement: Answer string-literal connection probes locally
The gateway SHALL answer a single SQL SELECT of one string literal, with
an optional column alias, as client chatter in Simple and Extended Query.
It SHALL return the exact decoded string as text without building the
metadata catalog or touching the database. This exception SHALL NOT
accept a FROM clause, expressions, parameters or multiple statements.
Other statements SHALL retain their existing routing and diagnostics.

#### Scenario: JDBC keep-alive probe
- **WHEN** a JDBC client executes `SELECT 'keep alive'`
- **THEN** it receives one text column and the exact value `keep alive`
  followed by `SELECT 1`, without a database statement

#### Scenario: Prepared probe preserves escaping and alias
- **WHEN** a client prepares `SELECT 'I''m alive' AS "KeepAlive"`
- **THEN** Describe reports one text column named `KeepAlive` and execution
  returns `I'm alive` in either text or binary format

#### Scenario: Probe exception does not route a table query
- **WHEN** a client sends `SELECT 'keep alive' FROM _Reference42`
- **THEN** it follows the SDBL path and is not answered as a probe
