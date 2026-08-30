## MODIFIED Requirements

### Requirement: Provide an interactive PostgreSQL REPL
The `open-sdbl-cli` package SHALL provide `open-sdbl repl postgres` using the
existing connection and authentication options. It SHALL load metadata at
startup, accept semicolon-terminated multiline UTF-8 queries, protect
interactive Linux terminal editing with `IUTF8`, recover from a byte-invalid
input line, and execute every generated statement in a verified read-only
`READ COMMITTED` transaction.

#### Scenario: Query execution
- **WHEN** the user enters a supported 1C query terminated by `;`
- **THEN** the CLI displays column labels, rows, and row count and then prompts
  for the next command

#### Scenario: Recoverable error
- **WHEN** compilation or PostgreSQL execution fails
- **THEN** the REPL prints the error, rolls back the statement transaction, and
  remains available

#### Scenario: Recoverable byte-invalid input
- **WHEN** one input line is not valid UTF-8
- **THEN** the REPL discards the affected statement, reports the input error,
  and accepts the next command without closing the database connection

#### Scenario: UTF-8 terminal editing
- **WHEN** the REPL runs on an interactive Linux terminal with `IUTF8` disabled
- **THEN** it enables `IUTF8` while reading commands and restores the previous
  terminal attributes before exit

#### Scenario: Session lifecycle
- **WHEN** input reaches EOF or the user enters `\q`
- **THEN** the CLI closes the PostgreSQL connection and exits successfully
