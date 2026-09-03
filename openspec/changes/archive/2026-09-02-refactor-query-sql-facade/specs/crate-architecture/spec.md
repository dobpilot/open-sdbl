## MODIFIED Requirements

### Requirement: Keep deterministic metadata work in the core
The `open-sdbl` library SHALL generate fixed metadata acquisition queries and
SHALL decode, resolve, and compile caller-provided metadata and SDBL without
opening a database connection. Query compilation SHALL be exposed through an
immutable generic `QueryCompiler<B>` bound to a separate PostgreSQL or MSSQL
backend value and backed by one shared functional core. Former database-named
free functions SHALL NOT remain part of the public API.

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
- **WHEN** an application requests PostgreSQL metadata query definitions or
  compiles SDBL through a provider compiler
- **THEN** the library returns deterministic SELECT-only SQL without executing
  it

### Requirement: Keep presentation policy outside the core crate
The core package SHALL define deterministic ID-only presentation requests,
validate structured plans, and generate SQL without adding production
dependencies. `QueryCompiler<B>` and its PostgreSQL/MSSQL backend values SHALL
remain immutable and perform no I/O. Application callback execution, async
coordination, and Moka caching SHALL belong to `open-sdbl-cli`. Both packages
SHALL use Rust Edition 2024 and declare an Edition-compatible MSRV.

#### Scenario: Core dependency graph
- **WHEN** another project builds only `open-sdbl`
- **THEN** no async runtime, PostgreSQL client, MSSQL client, or Moka cache
  dependency is compiled for the core package

#### Scenario: Provider object purity
- **WHEN** a generic compiler method is invoked repeatedly with equal source,
  snapshot, plans, and provider configuration
- **THEN** it produces equal results without mutating the facade or opening a
  connection

#### Scenario: Workspace edition
- **WHEN** Cargo reads either workspace package manifest
- **THEN** the package declares `edition = "2024"` and `rust-version` is at
  least 1.85
