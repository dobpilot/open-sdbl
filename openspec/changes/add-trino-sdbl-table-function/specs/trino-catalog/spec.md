## ADDED Requirements

### Requirement: Execute bounded SDBL SELECT through a Trino table function

The connector SHALL expose a polymorphic `system.sdbl` table function whose
VARCHAR argument is parsed and compiled as SDBL exclusively by the Rust core.
The function SHALL accept SELECT queries only and SHALL NOT accept generated or
user-provided PostgreSQL SQL over the connector protocol.

#### Scenario: Reference presentation

- **WHEN** a table-function query selects `Presentation`,
  `RefPresentation`, or the presentation property of a reference
- **THEN** Rust applies the configured presentation plans and PostgreSQL
  evaluates the compiled presentation in the remote SELECT

#### Scenario: Information-register slice

- **WHEN** a table-function query selects from a supported `SliceLast` or
  `SliceFirst` virtual table
- **THEN** Rust compiles its period, dimensions, and pre-ranking condition with
  the existing SDBL virtual-table semantics

#### Scenario: Accumulation-register virtual table

- **WHEN** a table-function query selects from a supported `Balance` or
  `Turnovers` virtual table
- **THEN** Rust compiles the existing metadata-aware aggregate relation and
  returns its virtual resources to Trino

#### Scenario: Dynamic result descriptor

- **WHEN** Trino analyzes a valid SDBL table-function invocation
- **THEN** the connector returns deterministic names and explicit Trino types
  for every output column without executing the query rows

#### Scenario: Projection and limit around SDBL

- **WHEN** Trino requests a subset of table-function columns or applies LIMIT
- **THEN** the Rust execution wrapper reads only requested output ordinals and
  applies the limit in PostgreSQL

#### Scenario: Invalid or mutating input

- **WHEN** the function argument is invalid, unsupported, or not one bounded
  SDBL SELECT
- **THEN** analysis fails with a positional compiler diagnostic and no
  PostgreSQL mutation is sent
