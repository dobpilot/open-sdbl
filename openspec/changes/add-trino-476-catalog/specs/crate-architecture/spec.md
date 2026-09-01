## MODIFIED Requirements

### Requirement: Separate the reusable library from runtime applications
The root `open-sdbl` package SHALL contain dependency-free reusable metadata,
lexer, and query logic. Database, process, environment, terminal, filesystem,
network, Trino SPI, async runtime, and service I/O SHALL be owned by workspace
application crates or the isolated Java connector module rather than the core
library.

#### Scenario: Core-only dependency
- **WHEN** a consumer builds only the `open-sdbl` library package
- **THEN** it requires no command-line target, async runtime, PostgreSQL client,
  HTTP server, Java runtime, or Trino SPI dependency

#### Scenario: Integration build
- **WHEN** a consumer builds the Trino integration targets
- **THEN** runtime dependencies remain confined to `open-sdbl-trino` and
  `trino-open-sdbl`, which reuse the core library through its public API
