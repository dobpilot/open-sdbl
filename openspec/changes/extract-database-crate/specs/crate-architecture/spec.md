## MODIFIED Requirements

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

### Requirement: Publish the workspace under MIT with aligned versions
Every package manifest in the repository SHALL declare `license = "MIT"`,
the repository SHALL ship the MIT license text naming the copyright holder,
and the `open-sdbl` library, the `open-sdbl-db` library and the
`open-sdbl-cli` application SHALL share the same semantic version so that a
CLI build names the library contract it was built against.

#### Scenario: Consistent manifests
- **WHEN** the manifests of the core library, the database library, the CLI,
  and the fuzz workspace are inspected
- **THEN** each declares the MIT license and the three workspace packages
  declare the same version

#### Scenario: Breaking library change
- **WHEN** a release changes the public API of `open-sdbl` incompatibly
- **THEN** the shared version number is raised in every package before the
  release is tagged

## ADDED Requirements

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
