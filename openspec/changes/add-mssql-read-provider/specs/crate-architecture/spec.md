## MODIFIED Requirements

### Requirement: Separate the reusable library from CLI applications
The repository SHALL expose `open-sdbl` as a library-only Cargo package and
SHALL place the `open-sdbl` executable in a separate `open-sdbl-cli` workspace
package. Database, process, environment, terminal, and filesystem I/O SHALL be
owned by the CLI package rather than the core library.

#### Scenario: Core-only dependency
- **WHEN** another Rust package depends on `open-sdbl`
- **THEN** it receives query-generation and decoding APIs without a binary
  target, async runtime, PostgreSQL client, or TDS client dependency

#### Scenario: CLI build
- **WHEN** a user builds package `open-sdbl-cli`
- **THEN** Cargo produces an executable named `open-sdbl` containing the `lex`,
  `metadata postgres`, `metadata mssql`, `console postgres`, and `console
  mssql` commands

### Requirement: Keep deterministic metadata work in the core
The `open-sdbl` library SHALL generate fixed metadata acquisition queries for
PostgreSQL and Microsoft SQL Server and SHALL decode and resolve caller-provided
DBNames, Config, SchemaStorage, and live-catalog records without opening a
database connection.

#### Scenario: Caller-provided resources
- **WHEN** an application supplies metadata blobs and typed live-catalog rows
- **THEN** the library resolves the metadata without reading environment
  variables, files, sockets, standard streams, or child processes

#### Scenario: PostgreSQL compiler
- **WHEN** an application constructs `QueryCompiler` from a metadata snapshot
  and `PostgresBackend`
- **THEN** the compiler can compile, prepare, and build deferred presentation
  lookups without provider runtime state

#### Scenario: MSSQL compiler
- **WHEN** an application constructs `QueryCompiler` with
  `MsSqlBackend::new(year_offset)`
- **THEN** the compiler consistently applies that immutable year offset to
  compilation, preparation, and presentation lookup operations

#### Scenario: Single public compilation model
- **WHEN** an application compiles or prepares a query for either backend
- **THEN** it uses `QueryCompiler<B>` rather than a parallel set of legacy free
  functions

#### Scenario: Query inspection
- **WHEN** an application requests PostgreSQL metadata query definitions
- **THEN** the library returns fixed SELECT-only statements without executing
  them

#### Scenario: MSSQL query inspection
- **WHEN** an application requests MSSQL metadata query definitions
- **THEN** the library returns fixed SELECT-only statements without executing
  them
