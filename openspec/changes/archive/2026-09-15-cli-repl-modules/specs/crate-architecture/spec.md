## ADDED Requirements

### Requirement: Carry one CLI feature per module
Every module of `open-sdbl-cli` SHALL carry one feature — one reason to
change. The console in particular SHALL be split so that the
read-execute loop, completion, statement preparation, deferred
presentations, meta commands, table rendering, metadata description and
terminal handling each live in a module of their own.

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
