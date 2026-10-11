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

### Requirement: Deliver a query result one row at a time
A database session SHALL offer a way to read a result row by row: each row
is decoded on its own and handed to the caller before the next is read
from the server. The caller SHALL be able to answer each row with
"continue" or "stop", and a "stop" SHALL end the read at once.

After a stop the session SHALL NOT read, decode, or allocate the rows that
follow. The row-collecting form of a query SHALL be defined in terms of
this one and SHALL keep the signature and the behaviour it has today.

#### Scenario: Stop after the rows the caller wanted
- **WHEN** a caller reads a result of many rows and stops after the first
  ten
- **THEN** exactly ten rows are decoded, and the source is not read past
  the point the caller stopped at

#### Scenario: Read to the end
- **WHEN** a caller answers "continue" to every row
- **THEN** it sees every row of the result, in the order the server sent
  them

#### Scenario: Failure of a row
- **WHEN** decoding a row fails, or the caller answers with an error
- **THEN** the read ends with that error and the transaction is ended the
  way a failed query ends it

#### Scenario: The collecting form is unchanged
- **WHEN** an application calls the row-collecting query
- **THEN** it receives the same rows, the same errors, and the same
  transaction behaviour as before

### Requirement: End a SQL Server statement by stopping the read
SQL Server is given no server-side execution limit by this crate —
`SET LOCK_TIMEOUT` bounds lock waiting only — so a statement that runs too
long cannot be ended by waiting. Stopping a streaming read on a SQL Server
session SHALL therefore drop the connection that carries the statement
rather than drain it, which ends the statement on the server. The session
SHALL report itself unusable afterwards, so that a caller reconnects
instead of sending on a stream left between protocol messages.

#### Scenario: Stopping ends the statement
- **WHEN** a caller stops a streaming read of a SQL Server result early
- **THEN** the connection carrying the statement is dropped and the
  session reports that it is no longer usable

#### Scenario: PostgreSQL keeps its session
- **WHEN** a caller stops a streaming read of a PostgreSQL result early
- **THEN** the transaction is ended and the session stays usable, because
  the server already bounds the statement by `statement_timeout`

### Requirement: Acquire a whole configuration in one read
The database package SHALL offer one session operation answering the
resolved metadata snapshot, the resolution report, the storage layout,
every resource of the `Config` table, and the resources of each
configuration extension. Every one of those SHALL be read inside the one
read-only transaction the metadata acquisition opens, through the same
`MetadataSource`, the same acquisition pipeline, the same part assembly
and the same decoding the existing read uses. There SHALL NOT be a second
metadata pipeline nor a second path to the database.

The resources the operation answers SHALL be the very resources metadata
resolution decoded. An implementation SHALL NOT resolve metadata, end the
transaction and then read the configuration again.

The operation SHALL NOT claim that what it answers is one state of the
base. The transaction is `READ COMMITTED`, under which each statement
sees the rows committed when it began, so a base written to during the
acquisition can answer one statement from before that write and the next
from after it. The operation removes the *second* read a consumer would
otherwise perform after the session ended — it does not make the reads
atomic. The documentation SHALL say so. It SHALL restrict CONFIG_FINGERPRINT
to Config rows with a bare GUID or a .1c/.9/.7 suffix. Other Config resources
and ConfigCAS/_ExtensionsInfo/_ExtensionsRestruct SHALL be documented as
excluded. Equal fingerprints SHALL NOT be presented as proof that a complete
or selected configuration acquisition, extensions or resolved snapshot
remained unchanged.

The result SHALL be answered only after the whole acquisition succeeds.
Any failure SHALL go through the rollback the provider already performs
and SHALL answer that error; no partial result SHALL be answered.

Resource bytes SHALL be answered as the base stores them — compressed,
unmodified — and the type answering them SHALL document that. Bytes read
once SHALL be shared rather than copied per holder, so that a resource
two extensions both name is held once.

The operation, the type it answers, and the type of one acquired
extension SHALL be re-exported from the package root.

#### Scenario: Metadata and resources come from one read
- **WHEN** the operation runs against a base
- **THEN** it answers the same snapshot and the same report the metadata
  read answers, together with the resources those were resolved from,
  without reading `Config` a second time

#### Scenario: A base written to during the read
- **WHEN** a consumer needs to know whether the configuration moved while
  it was being read
- **THEN** the documentation explains READ COMMITTED and the limited
  metadata-Config fingerprint boundary, without promising whole-acquisition
  freshness or atomicity

#### Scenario: A resource the metadata read ignores
- **WHEN** the `Config` table carries a resource the acquisition filter
  rejects
- **THEN** the answered resources include it, and the snapshot is
  unchanged by it

#### Scenario: A failure part way through
- **WHEN** reading one resource fails
- **THEN** the transaction is rolled back, the error is answered, and no
  configuration is answered

#### Scenario: Excluded resource or extension changes
- **WHEN** only an ineligible Config resource or extension data changes
- **THEN** the documentation permits CONFIG_FINGERPRINT to remain equal
  and does not claim the whole configuration stayed unchanged

### Requirement: Keep each acquired extension apart
Each extension the operation answers SHALL carry its identity, its name,
whether the base applies it, the order the platform applies it in, and
its own resources. Resources SHALL be attributed to an extension through
that extension's own root index, so that two extensions naming the same
resource keep two entries rather than one shared entry. The answered
order SHALL be the platform's application order, so that a consumer can
apply the active extensions in it.

#### Scenario: Two extensions
- **WHEN** a base carries two extensions
- **THEN** each is answered with its own identity, name, order and
  resource list

#### Scenario: A name both extensions use
- **WHEN** both extensions name a resource identically
- **THEN** each keeps its own entry under that name and neither list
  gains the other's

#### Scenario: An extension the base does not apply
- **WHEN** an extension is present but not active
- **THEN** it is answered with its resources and marked inactive, so that
  a consumer can skip it

#### Scenario: An extension whose record is not understood
- **WHEN** the record of an extension does not carry the applicability
  flag where it was measured
- **THEN** the extension is answered with its activity unknown rather
  than guessed, so that a consumer cannot silently skip one the base
  applies

### Requirement: Bound what a whole-configuration read retains
The limits a session works under SHALL carry a ceiling on how many
`Config` resources a whole-configuration read may retain and on how many
compressed bytes it may hold. The read SHALL check the totals the server
reports against those ceilings before it reads a row, and SHALL check
again as it assembles, because the totals are a statement of their own
and the table may have grown since. Exceeding either SHALL be a typed
error naming the ceiling, not an exhausted process.

#### Scenario: A base larger than the ceiling
- **WHEN** the totals statement reports more resources or more bytes than
  the limits allow
- **THEN** the read fails with a typed error naming the ceiling before it
  reads the first resource row

#### Scenario: A table that grew after the totals
- **WHEN** the rows read exceed the ceiling although the totals did not
- **THEN** the read fails with the same typed error instead of retaining
  them

### Requirement: Keep the metadata read cheap
The existing metadata read SHALL keep its signature, its result and its
cost. It SHALL NOT begin to answer the resources of the configuration and
SHALL NOT be required to hold them: it SHALL keep streaming the filtered
`Config` resources and discarding each after decoding. Sharing the
acquisition pipeline with the whole-configuration read SHALL NOT change
what it reads or what it retains.

#### Scenario: Reading only the metadata
- **WHEN** an application asks for the metadata of a base
- **THEN** the same statements run as before, the whole-table statement
  does not, and no resource is retained after it is decoded

### Requirement: Drive one session from synchronous code
`open-sdbl-db` SHALL offer `BlockingDatabaseSession`, a value owning one
single-threaded Tokio runtime and one `DatabaseSession`, with blocking
`connect`, `configuration`, `metadata`, `query`, `query_each` and `close`.
Each blocking operation SHALL drive the asynchronous `DatabaseSession`
operation of the same name and SHALL NOT carry a statement, a provider
type or decoding of its own, so that limits, timeouts, diagnostics,
session poisoning and provider behaviour are those of the asynchronous
session.

Each value SHALL own its own runtime and its own connection, so that
several values opened side by side are independent and closing one leaves
the others usable. The value SHALL be `Send`.

`close` SHALL take the session out of the value. Every operation after it,
a second `close` included, SHALL answer `DbError::SessionClosed`, whether
the first `close` succeeded or failed. An operation called on a thread that
is already driving a Tokio runtime SHALL answer `DbError::AsyncContext`
rather than panic.

The type SHALL NOT know about any consumer, programming language, session
parameter or command line; the package root SHALL re-export it.

#### Scenario: Same answer both ways
- **WHEN** the same artificial provider answers a configuration read, a
  query and a failing query once through the asynchronous path and once
  through the blocking driver
- **THEN** the results are equal and the errors are the same error

#### Scenario: Two sessions
- **WHEN** two blocking sessions are opened and the first is closed
- **THEN** they were two connections, and the second still answers

#### Scenario: A call after close
- **WHEN** any operation, or `close` again, is called after `close`
- **THEN** it answers `DbError::SessionClosed` and does not panic

#### Scenario: Called from asynchronous code
- **WHEN** a blocking operation is called from inside a Tokio runtime
- **THEN** it answers `DbError::AsyncContext` and does not panic

#### Scenario: No second SQL path
- **WHEN** the blocking module is inspected
- **THEN** it names no statement, no provider session and no driver type

### Requirement: Build credentials from owned strings
`Credentials` SHALL offer a constructor taking an owned `Option<String>`
for each of the PostgreSQL, SQL Server and SOCKS5 secrets, and
`EnvironmentSecret` SHALL offer one taking an owned `Option<String>`. Each
SHALL move the string into zeroized memory inside the package, so that a
caller never has to name the zeroizing crate.

`Credentials` and `EnvironmentSecret` SHALL implement `Debug` without ever
printing a secret. `DatabaseConnection` SHALL carry no secret, and no error
the package answers SHALL contain one.

#### Scenario: A caller without zeroize
- **WHEN** a caller builds credentials from `Some(String)` and `None`
- **THEN** the PostgreSQL secret is present, the absent ones are missing,
  and the caller's code names no type of the zeroizing crate

#### Scenario: A password in a debug print
- **WHEN** credentials holding a password are formatted with `{:?}`
- **THEN** the output names which secrets are present and does not
  contain the password

#### Scenario: A password and a refused connection
- **WHEN** a blocking session connects with a password to a port nothing
  listens on
- **THEN** it answers a connection error whose text and debug form do not
  contain the password

### Requirement: Describe each acquired extension
Each extension a whole-configuration read answers SHALL carry, beside its
identity, name, activity, order and resources: its version, the content
hash of its root, whether safe mode is on, whether it is used in a
distributed information base, its scope, the catalog of the classes and
objects its configuration declares, and the object headers of its
descriptors with the explicit link an adopted object carries to the
main-configuration object it adopts.

A value the reader does not decode reliably SHALL be an `Option` and SHALL
be `None` rather than a default: the version when the record carries none,
safe mode for unsupported records, the distributed-base flag and the scope
when the base lacks the column, the catalog when the configuration root is
missing or not in the measured shape, and an object's link when it is
nil. The link SHALL be read from the adopted object's own header; the name
a resource has in `ConfigCAS` SHALL NOT be taken as a link. Both providers
SHALL answer the same row type and share every step after it.

The object headers SHALL be answered whole or not at all, as an `Option`
of the list. The list SHALL be `Some` only when every description
resource of the extension — every resource its root index names by a bare
identifier — inflated within the per-resource limit and parsed, the
catalog is known, and every object the catalog declares has a header of
its own read from the resource named by that object. In every other case
the list SHALL be `None`; a shorter list SHALL NOT be answered as the
list. Resources whose name carries a suffix are not descriptions and SHALL
NOT affect completeness. Completeness SHALL add no data: a header whose
tail is not in the measured shape keeps its belonging and link unknown,
and an adopted object recording the nil identifier keeps no link.

#### Scenario: An applied and a not-applied extension
- **WHEN** a base carries one extension it applies and one it does not
- **THEN** both are answered with identity, order, activity `Some(true)`
  and `Some(false)`, and their own content hash

#### Scenario: An adopted catalog
- **WHEN** an extension adopts a catalog of the main configuration by
  identifier
- **THEN** its object list carries that catalog as adopted, with the
  identifier of the main-configuration catalog as its link

#### Scenario: An object of the extension's own
- **WHEN** an extension declares a catalog of its own
- **THEN** that object is answered as own, without a link

#### Scenario: A configuration root that is not understood
- **WHEN** the configuration root resource of an extension does not have
  the measured shape
- **THEN** the extension is answered with no catalog rather than a partial
  one and no object headers, since there is nothing to check them
  against, and everything else as before

#### Scenario: Safe mode
- **WHEN** an extension is read in the measured complete runtime-property record shape
- **THEN** safe mode is the decoded Boolean

#### Scenario: Unsupported safe mode record
- **WHEN** the extension's runtime-property record is truncated or unsupported
- **THEN** safe mode is unknown

#### Scenario: A base without the new columns
- **WHEN** `_ExtensionsInfo` has neither `_ExtensionScope` nor
  `_UsedInDistributedInfoBase`
- **THEN** the extensions are read, with scope and distributed-base use
  unknown

#### Scenario: Every description decoded
- **WHEN** every description resource of an extension decodes and every
  declared object has its header
- **THEN** the object headers are answered, all of them

#### Scenario: A damaged description
- **WHEN** one description resource of an extension does not inflate or
  does not parse
- **THEN** the object headers are unknown, rather than answered without
  that resource's headers

#### Scenario: A declared object without a header
- **WHEN** the catalog declares an object whose own resource carries no
  header for it
- **THEN** the object headers are unknown

#### Scenario: A damaged resource that is no description
- **WHEN** a resource whose name carries a suffix, such as a module, does
  not decode
- **THEN** the object headers are answered as if it were intact

### Requirement: Acquire selected configuration resources
The database crate SHALL let a consumer select Config resources to retain after it has inspected the `root` resource and the configuration descriptor named there. Both bootstrap resources SHALL be retained. Every other Config resource SHALL still be decoded when required for metadata resolution, while a resource not selected SHALL be discarded after decoding. The selected read SHALL preserve resolved metadata, extension information, rollback on failure, and the per-store retention limits of the whole read. The existing whole-configuration and metadata-only reads SHALL keep their contracts.

#### Scenario: Consumer selects code and object resources
- **WHEN** the consumer selects names from the configuration descriptor and excludes form assets
- **THEN** the answer includes both bootstrap resources and every selected resource, resolves the same metadata, and retains no excluded form asset

#### Scenario: Selection fails
- **WHEN** a bootstrap resource is absent or the selector rejects it
- **THEN** acquisition fails and rolls back the read-only transaction

### Requirement: Read access through a blocking session
A blocking session SHALL offer user and role-right reads through the existing asynchronous access functions on its owned database session and runtime. It SHALL create no extra connection or independent SQL path.

#### Scenario: Closed or asynchronous context
- **WHEN** an access read is called on a closed blocking session, including inside an asynchronous runtime
- **THEN** it answers `SessionClosed`
- **WHEN** the same read is called on an open blocking session inside an asynchronous runtime
- **THEN** it answers `AsyncContext` and leaves the session open

### Requirement: Answer measured native extension runtime properties
Acquired extensions SHALL carry the native platform hash sum separately from the existing hexadecimal content hash, the decoded safe-mode Boolean, the security profile name and a typed known scope alongside the raw scope code. Known empty profile names SHALL remain distinguishable from unknown profiles. Unknown, truncated or structurally unsupported mode/profile records SHALL return unknown values rather than guessed defaults. Counted profile strings SHALL NOT be treated as extension versions.

#### Scenario: Safe mode is explicitly disabled
- **WHEN** a complete measured record encodes safe mode false and no profile
- **THEN** its mode is false and its known profile name is empty

#### Scenario: A named security profile
- **WHEN** a complete measured record carries a security profile name
- **THEN** the profile string is preserved separately from the mode Boolean and version

#### Scenario: The native generation hash
- **WHEN** a record is in the measured native generation-key shape
- **THEN** the platform hash sum uses the measured Base64 generation key representation, independently of the hex CAS content hash

#### Scenario: The measured InfoBase scope
- **WHEN** the raw scope code is 1
- **THEN** its known scope is InfoBase

#### Scenario: An unmeasured scope
- **WHEN** the raw scope code has not been mapped on the native platform
- **THEN** its raw value remains available and its typed scope is unknown

#### Scenario: A malformed property record
- **WHEN** a mode/profile field or the record's structural walk is incomplete or unsupported
- **THEN** mode and profile are unknown

### Requirement: Distinguish a known runtime version from an unknown record
Acquired extensions SHALL provide an authoritative runtime version separately from legacy partial decoding. A complete measured record with an absent version SHALL return a known empty string. Unsupported or incomplete records SHALL return an unknown value.

#### Scenario: No configured version
- **WHEN** a complete measured record contains the native absent-version marker
- **THEN** its runtime version is known and empty

#### Scenario: Unsupported record
- **WHEN** the runtime record is incomplete or unsupported
- **THEN** its runtime version is unknown even if legacy decoding found a printable field

#### Scenario: Configured version
- **WHEN** a complete measured record carries a version string
- **THEN** that string is preserved as its runtime version

### Requirement: Decode measured legacy extension object headers
Extension object acquisition SHALL recognize legacy header UUID and name shapes confirmed by native XML export. The measured Adopted belonging SHALL be retained. A modified-property identifier SHALL NOT be reported as an adoption target. Existing whole-or-unknown object-list completeness SHALL remain unchanged.

#### Scenario: Native legacy adopted object
- **WHEN** a descriptor uses the measured legacy header and Adopted tail
- **THEN** its own UUID, name and Adopted belonging are decoded without an inferred target link

#### Scenario: Unsupported legacy tail
- **WHEN** a recognized legacy identity and name have an unsupported tail
- **THEN** the identity and name remain available but belonging and target link remain unknown

#### Scenario: Missing own header
- **WHEN** any catalog object still lacks its own decoded header
- **THEN** the acquired object list remains unknown rather than partial

### Requirement: Acquire a measured extension purpose
Acquired extensions SHALL expose an optional typed ConfigurationExtensionPurpose with Patch, Customization and AddOn variants. Purpose SHALL come only from native-measured extension data through the shared acquisition decoder. It SHALL remain independent of extension name, activation, scope and version. Acquisition SHALL remain read-only and SHALL NOT add per-BSL-session reads or external dependencies.

#### Scenario: Three measured purposes
- **WHEN** acquisition reads complete native-measured Patch, Customization or AddOn data
- **THEN** it exposes the corresponding typed purpose on either supported SQL provider

#### Scenario: Unknown or damaged data
- **WHEN** the purpose source is unavailable, unsupported, unknown or malformed
- **THEN** purpose is None and unrelated reliably acquired fields remain available

#### Scenario: Other properties change
- **WHEN** an extension name, activation, scope or version changes without changing purpose
- **THEN** its acquired purpose remains unchanged
