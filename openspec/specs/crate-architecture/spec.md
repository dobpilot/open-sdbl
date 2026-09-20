# crate-architecture Specification

## Purpose
Define the Cargo workspace boundary that keeps deterministic SDBL and metadata
logic reusable while isolating command-line and database I/O in application
crates.

## Requirements

### Requirement: Separate the reusable library from CLI applications
The repository SHALL expose `open-sdbl` as a library-only Cargo package,
SHALL expose the database layer as a second library-only Cargo package
`open-sdbl-db`, and SHALL place the `open-sdbl` executable in a separate
`open-sdbl-cli` workspace package. Database and network I/O SHALL be owned
by `open-sdbl-db`; process, environment, terminal, and filesystem I/O SHALL
be owned by the CLI package. Neither belongs to the core library.

#### Scenario: Core-only dependency
- **WHEN** another Rust package depends on `open-sdbl`
- **THEN** it receives query-generation and decoding APIs without a binary
  target, async runtime, PostgreSQL client, or TDS client dependency

#### Scenario: Database-layer dependency
- **WHEN** another Rust package depends on `open-sdbl-db`
- **THEN** it can connect to a base, read its metadata, read the rights of
  roles and derive the restrictions of a user without depending on the CLI
  package and without the CLI writing anything to a terminal

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
and the `open-sdbl` library, the `open-sdbl-db` library and the
`open-sdbl-cli` application SHALL share the same semantic version so that a
CLI build names the library contract it was built against.

The two libraries — `open-sdbl` and `open-sdbl-db` — SHALL be publishable,
so an application outside this repository can depend on them from a
registry rather than from a checkout. Every dependency of a publishable
package on another package of this workspace SHALL name the shared version
beside the path. The `open-sdbl-cli` application and the fuzz package are
binaries of this repository and SHALL declare `publish = false`.

#### Scenario: Consistent manifests
- **WHEN** the manifests of the core library, the database library, the CLI,
  and the fuzz workspace are inspected
- **THEN** each declares the MIT license and the three workspace packages
  declare the same version

#### Scenario: Breaking library change
- **WHEN** a release changes the public API of `open-sdbl` incompatibly
- **THEN** the shared version number is raised in every package before the
  release is tagged

#### Scenario: The database library may be published
- **WHEN** the manifest of `open-sdbl-db` is inspected
- **THEN** it declares no `publish = false`, and its dependency on
  `open-sdbl` names the shared version as well as the path, so the package
  can be packaged once `open-sdbl` is in the registry it names

#### Scenario: The application stays unpublished
- **WHEN** `cargo publish -p open-sdbl-cli` is attempted
- **THEN** Cargo refuses, because the application declares `publish = false`

### Requirement: Keep CLI tests aligned with module ownership
Tests for provider, argument, credential, network, output, progress, and
pipeline behavior SHALL live beside the production module that owns that
behavior, while the binary root SHALL retain only orchestration-level tests.

#### Scenario: Locate a CLI behavior test
- **WHEN** a maintainer changes behavior owned by a focused CLI module
- **THEN** its unit tests are discoverable in that module without depending on
  private imports collected by the binary root

### Requirement: Keep the CLI binary root an entry point
The binary root of `open-sdbl-cli` SHALL contain the process entry point
and the module declarations only: parsing the command line, starting the
runtime, and reporting the exit status. Connecting to a database, reading
metadata, running a command and holding shared limits SHALL live in
modules of their own.

No module of the CLI SHALL import an item from the binary root: a shared
constant belongs to the module that owns it, so that the database layer
does not depend on the binary that drives it.

#### Scenario: Locate the session facade
- **WHEN** a maintainer looks for the code that connects to a provider and
  reads its metadata
- **THEN** it is found in the session module, not in the binary root

#### Scenario: Shared limit
- **WHEN** the database layer needs the connection timeout
- **THEN** it imports it from the module that owns the limits, not from
  the crate root

### Requirement: Carry one CLI feature per module
Every module of `open-sdbl-cli` SHALL carry one feature — one reason to
change. The console in particular SHALL be split so that the
read-execute loop, completion, statement preparation, deferred
presentations, meta commands, table rendering, metadata description and
terminal handling each live in a module of their own, and each database
provider SHALL be split so that driving a session, reading metadata and
decoding provider values live in modules of their own.

Unit tests SHALL live outside the implementation files, in `src/tests/`,
one file per module they cover.

#### Scenario: Change the table rendering
- **WHEN** a maintainer changes how a result table is printed
- **THEN** the change touches the rendering module alone, not the module
  that talks to the database

#### Scenario: Locate the tests of a module
- **WHEN** a maintainer looks for the unit tests of a CLI module
- **THEN** they are found in `src/tests/` under that module's name, and the
  implementation file contains no test code

#### Scenario: Decode a provider value
- **WHEN** a maintainer changes how a provider value becomes a `Cell`
- **THEN** the change touches that provider's decoding module, which needs
  no connection to be read or tested

### Requirement: Publish the release build as CI artifacts
A continuous-integration run SHALL leave its release build behind: an
archive of the commit's sources and the CLI executable built for
`x86_64-pc-windows-msvc`, both published as artifacts of that run and
named after the version the manifests declare.

The source archive SHALL contain what the repository tracks at that
commit, without build output, and SHALL unpack into a directory named
after the version.

#### Scenario: Take the sources of a run
- **WHEN** a maintainer opens a finished CI run
- **THEN** a `.tar.gz` of the sources is attached, unpacking into a
  version-named directory

#### Scenario: Take the Windows executable
- **WHEN** the same run is opened
- **THEN** the CLI executable built for Windows MSVC is attached, so trying
  it needs no local Rust toolchain

### Requirement: Own the database layer in a library package
`open-sdbl-db` SHALL own connecting to PostgreSQL and Microsoft SQL Server,
the read-only transaction discipline, metadata acquisition and decoding,
the SOCKS5 transport, result-cell decoding, the users and rights of roles,
the expansion of access restrictions, the restriction store, the
session-parameter cache of the Standard Subsystems Library, and the index
of configuration extensions. Every item an application needs SHALL be
public and documented in English; a public enumeration that may gain a
variant SHALL be `#[non_exhaustive]`, while one that enumerates a closed
set — the two providers this crate speaks to — SHALL NOT, so that adding
a provider fails to compile at every place that must handle it.

#### Scenario: Read the restrictions of a user
- **WHEN** an application connects with `DatabaseSession::connect`, reads
  the metadata, and asks for the restrictions of a named user
- **THEN** it receives the same expanded restriction texts the console
  shows, without linking the CLI package

#### Scenario: Documented surface
- **WHEN** `cargo doc --workspace --no-deps` runs with `RUSTDOCFLAGS="-D warnings"`
- **THEN** it completes without warnings, so every public item of the
  database package carries documentation

### Requirement: Keep terminal and command-line policy out of the database package
`open-sdbl-db` SHALL NOT print what it answers, SHALL NOT parse a command
line, and SHALL NOT read the process environment or a password file.
Metadata acquisition SHALL return the resolution report to its caller
instead of printing it. Progress SHALL be reported through a trait whose
methods default to doing nothing, so that an application that wants no
progress implements nothing. Connection descriptions and the credential
carrier SHALL be plain library types the CLI fills in.

Two `warning:` lines about resources the Config decoder had to skip are
written to standard error as they were before the extraction; turning them
into reported findings is a change of its own.

#### Scenario: Acquire metadata without a terminal
- **WHEN** an application acquires metadata through the database package
- **THEN** it receives the snapshot, the storage layout and the resolution
  report as values, and the package prints none of them

#### Scenario: Silent progress
- **WHEN** an application passes a progress reporter that overrides no method
- **THEN** metadata acquisition runs to completion with no progress output

#### Scenario: Credentials supplied by the caller
- **WHEN** the CLI reads `PGPASSWORD`, `MSSQL_PASSWORD`, `SOCKS5_PASSWORD`
  and `~/.pgpass`
- **THEN** it hands the database package a credential value holding the
  secrets in zeroized memory, and the package reads no environment variable
  of its own

### Requirement: Carry the shared limits in one value
The time and size limits the database layer applies — the connection
timeout, the per-call query timeout, the PostgreSQL close timeout and the
Config decoding batch size — SHALL be fields of one public `Limits` value
whose `Default` implementation yields the values the console used before
the extraction. A session SHALL apply the limits it was opened with.

#### Scenario: Default limits
- **WHEN** the CLI opens a session without naming limits
- **THEN** the connection timeout is 10 seconds, one server call may run for
  120 seconds, the PostgreSQL driver is given 5 seconds to wind down, and
  Config resources are decoded 256 per round trip

#### Scenario: Caller-chosen limits
- **WHEN** an application opens a session with a shorter query timeout
- **THEN** a server call that exceeds it fails with the timeout error naming
  that duration
