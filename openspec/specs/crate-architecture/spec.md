# crate-architecture Specification

## Purpose
Define the Cargo workspace boundary that keeps deterministic SDBL and metadata
logic reusable while isolating command-line and database I/O in application
crates.

## Requirements

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
PostgreSQL and Microsoft SQL Server and SHALL decode, resolve, and compile
caller-provided DBNames, Config, SchemaStorage, live-catalog records, and SDBL
without opening a database connection. Query compilation SHALL be exposed
through an immutable generic `QueryCompiler<B>` bound to a separate PostgreSQL
or MSSQL backend value and backed by one shared functional core. Compilation
inputs beyond the source text (presentation plans and named parameter
values) SHALL be passed through one `CompileOptions` value accepted by
`QueryCompiler::compile_with` and `Prepared::compile_with`; the existing
`compile`, `compile_with_presentations`, and `Prepared::compile` methods
SHALL remain equivalent to default options. Temporary-table state SHALL be
carried by one public `TempTablesManager` value passed as `&mut` to
`QueryCompiler::compile_batch` and `Prepared::compile_batch` and as `&` to
`QueryCompiler::prepare_with`; the manager SHALL hold only compiled text
and column metadata and SHALL perform no I/O. Former database-named free
functions SHALL NOT remain part of the public API.

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

#### Scenario: Compilation options
- **WHEN** an application builds `CompileOptions::new().presentations(&plans).parameters(&params)`
  and calls `compile_with`
- **THEN** the compiler applies both inputs, and calling `compile` on the
  same source without parameters yields the same SQL when the source has no
  parameters

#### Scenario: Batch compilation with a manager
- **WHEN** an application creates `TempTablesManager::new()`, calls
  `compile_batch` on a batch that places a table, and then `compile_batch`
  on a query reading it
- **THEN** the second call succeeds with the definition emitted as a CTE,
  and neither call touches the file system, environment, or network

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

### Requirement: Build the CLI application by default
The workspace SHALL select `open-sdbl-cli` as its default Cargo member. A Cargo
build invoked from the repository root without `--package` or `--workspace`
SHALL build the CLI application and its `open-sdbl` library dependency while
preserving explicit library-only and whole-workspace build selection.

#### Scenario: Default release build
- **WHEN** a user runs `cargo build --release` from a clean repository checkout
- **THEN** Cargo produces the `target/release/open-sdbl` executable

#### Scenario: Explicit library-only build
- **WHEN** a user runs `cargo build --release --package open-sdbl`
- **THEN** Cargo builds the dependency-free library without requiring the CLI
  application target

#### Scenario: Explicit whole-workspace build
- **WHEN** a user runs `cargo build --release --workspace`
- **THEN** Cargo builds both workspace packages and produces the CLI executable

### Requirement: Expose one sealed backend abstraction
Query compilation SHALL be reachable through a sealed backend trait
implemented by the PostgreSQL and MSSQL backend values, so applications can
write backend-generic code against `QueryCompiler<B>` and a single generic
prepared-query type, without the crate committing to open extension.

#### Scenario: Backend-generic application code
- **WHEN** an application writes one function generic over the backend
  trait and calls it with the PostgreSQL and the MSSQL backend
- **THEN** both calls compile queries through the same generic API surface

#### Scenario: Sealed extension point
- **WHEN** an external crate attempts to implement the backend trait for
  its own type
- **THEN** the implementation is rejected at compile time

### Requirement: Keep source modules bounded
The query-compilation implementation SHALL be organized into focused
submodules (diagnostics, AST, parser, dialect, metadata resolution, code
generation) rather than one monolithic module, and shared compilation logic
SHALL exist exactly once regardless of how many sources a query joins.

#### Scenario: Single- and multi-source queries share logic
- **WHEN** the same dereference expression is compiled in a single-source
  query and in a JOIN query
- **THEN** both paths execute the same resolution and join-planning code
  and produce consistent SQL

### Requirement: Publish the workspace under MIT with aligned versions
Every package manifest in the repository SHALL declare `license = "MIT"`,
the repository SHALL ship the MIT license text naming the copyright holder,
and the `open-sdbl` library and `open-sdbl-cli` application SHALL share the
same semantic version so that a CLI build names the library contract it was
built against.

#### Scenario: Consistent manifests
- **WHEN** the manifests of the library, the CLI, and the fuzz workspace are
  inspected
- **THEN** each declares the MIT license and the library and CLI declare the
  same version

#### Scenario: Breaking library change
- **WHEN** a release changes the public API of `open-sdbl` incompatibly
- **THEN** the shared version number is raised in both packages before the
  release is tagged
