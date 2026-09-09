## MODIFIED Requirements

### Requirement: Keep deterministic metadata work in the core
The `open-sdbl` library SHALL generate fixed metadata acquisition queries for
PostgreSQL and Microsoft SQL Server and SHALL decode, resolve, and compile
caller-provided DBNames, Config, SchemaStorage, live-catalog records, and SDBL
without opening a database connection. Query compilation SHALL be exposed
through an immutable generic `QueryCompiler<B>` bound to a separate PostgreSQL
or MSSQL backend value and backed by one shared functional core. Former
database-named free functions SHALL NOT remain part of the public API.

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
  `MsSqlBackend::new(year_offset)` optionally followed by
  `with_dialect_level(level)`
- **THEN** the compiler consistently applies that immutable year offset and
  dialect level to compilation, preparation, and presentation lookup
  operations

#### Scenario: Single public compilation model
- **WHEN** an application compiles or prepares a query for either backend
- **THEN** it uses `QueryCompiler<B>` rather than a parallel set of legacy free
  functions

#### Scenario: Query inspection
- **WHEN** an application prepares a query with `QueryCompiler`
- **THEN** it can inspect the presentation request before supplying plans and
  compiling them

#### Scenario: MSSQL query inspection
- **WHEN** an application prepares a query with `QueryCompiler` bound to
  `MsSqlBackend`
- **THEN** it can inspect the presentation request before supplying plans and
  compiling them
