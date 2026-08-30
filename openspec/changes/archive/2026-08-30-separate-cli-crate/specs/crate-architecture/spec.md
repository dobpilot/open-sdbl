## ADDED Requirements

### Requirement: Separate the reusable library from CLI applications
The repository SHALL expose `open-sdbl` as a library-only Cargo package and
SHALL place the `open-sdbl` executable in a separate `open-sdbl-cli` workspace
package. Database, process, environment, terminal, and filesystem I/O SHALL be
owned by the CLI package rather than the core library.

#### Scenario: Core-only dependency
- **WHEN** another Rust package depends on `open-sdbl`
- **THEN** it receives query-generation and decoding APIs without a binary
  target, async runtime, or PostgreSQL client dependency

#### Scenario: CLI build
- **WHEN** a user builds package `open-sdbl-cli`
- **THEN** Cargo produces an executable named `open-sdbl` containing the `lex`
  and `metadata postgres` commands

### Requirement: Keep deterministic metadata work in the core
The `open-sdbl` library SHALL generate the fixed metadata acquisition queries
and SHALL decode and resolve caller-provided DBNames, Config, SchemaStorage, and
live-catalog records without opening a database connection.

#### Scenario: Caller-provided resources
- **WHEN** an application supplies metadata blobs and typed live-catalog rows
- **THEN** the library resolves the metadata without reading environment
  variables, files, sockets, standard streams, or child processes

#### Scenario: Query inspection
- **WHEN** an application requests PostgreSQL metadata query definitions
- **THEN** the library returns fixed SELECT-only statements without executing
  them
