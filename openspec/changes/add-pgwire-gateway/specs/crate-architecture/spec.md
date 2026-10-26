## ADDED Requirements

### Requirement: Isolate the wire-protocol gateway in its own package
The repository SHALL expose the PostgreSQL wire-protocol gateway as a
library-only workspace package `open-sdbl-pgwire` depending on `open-sdbl`
and `open-sdbl-db`, and the CLI SHALL expose it as the `serve` command.
Neither `open-sdbl` nor `open-sdbl-db` SHALL depend on it.

#### Scenario: Embedding the gateway
- **WHEN** another Rust application depends on `open-sdbl-pgwire`
- **THEN** it can serve SDBL over the PostgreSQL protocol on its own
  listener without depending on the CLI package

#### Scenario: Core stays independent
- **WHEN** a package depends only on `open-sdbl`
- **THEN** it receives no async runtime, TLS, or gateway code

#### Scenario: CLI build includes serve
- **WHEN** a user builds package `open-sdbl-cli`
- **THEN** the `open-sdbl` executable also contains the `serve postgres`
  and `serve mssql` commands

### Requirement: Keep host I/O out of the gateway package
`open-sdbl-pgwire` SHALL own only the listening socket, client connections
and database sessions. It SHALL NOT read environment variables, files or
the command line and SHALL NOT write to a terminal; the caller supplies the
listener, connection description, `Credentials`, `Limits`, parsed TLS
configuration and session parameters.

#### Scenario: Certificates from the caller
- **WHEN** an application starts the gateway with TLS
- **THEN** it passes an already-built rustls server configuration and the
  gateway opens no file

### Requirement: Add no new third-party dependency for the gateway
`open-sdbl-pgwire` SHALL use only third-party production dependencies that
are already in the workspace dependency tree; the protocol codec SHALL be
implemented in the package.

#### Scenario: Dependency tree
- **WHEN** `cargo tree` is compared before and after the change
- **THEN** no new third-party crate appears among production dependencies
