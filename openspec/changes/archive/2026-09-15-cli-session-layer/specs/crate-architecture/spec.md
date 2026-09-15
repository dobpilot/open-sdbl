## ADDED Requirements

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
