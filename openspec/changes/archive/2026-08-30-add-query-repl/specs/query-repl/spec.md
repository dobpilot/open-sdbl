## ADDED Requirements

### Requirement: Compile a bounded read-only 1C query subset
The `open-sdbl` library SHALL compile a single-source
`ВЫБРАТЬ`/`SELECT` query into a PostgreSQL SELECT using only authoritative
resolved metadata. The supported grammar SHALL include projection or `*`, one
metadata source, optional source alias, `РАЗЛИЧНЫЕ`/`DISTINCT`,
`ПЕРВЫЕ`/`TOP`, basic `ГДЕ`/`WHERE` expressions, and
`УПОРЯДОЧИТЬ ПО`/`ORDER BY` with `ВОЗР`/`ASC` and `УБЫВ`/`DESC` directions.
Unsupported syntax SHALL fail before execution.

#### Scenario: Logical catalog query
- **WHEN** a query selects `Код` and a custom attribute from
  `Справочник.<name>`
- **THEN** generated SQL uses the DBNames-resolved table and Config-resolved
  physical columns without inferring a numeric name

#### Scenario: Bounded syntax failure
- **WHEN** a query contains a join, mutation, temporary table, parameter, or
  unsupported clause
- **THEN** compilation returns a positional diagnostic and no SQL is produced

### Requirement: Resolve queryable objects and fields bilingually
The compiler SHALL accept Russian and English metadata-kind names and standard
field names, Config descriptor field names, unique bare object names for
inspection, and exact canonical physical table names for inspection. Ambiguous
or missing names SHALL be diagnosed rather than guessed.

#### Scenario: Standard field alias
- **WHEN** a catalog query refers to `Код`, `Наименование`, or `Ссылка`
- **THEN** the compiler resolves the corresponding live `Code`, `Description`,
  or `ID` physical representation

#### Scenario: Compound field projection
- **WHEN** a selected logical field has multiple physical representation
  members
- **THEN** generated SQL projects every member with a stable logical label

### Requirement: Provide an interactive PostgreSQL REPL
The `open-sdbl-cli` package SHALL provide `open-sdbl repl postgres` using the
existing connection and authentication options. It SHALL load metadata at
startup, accept semicolon-terminated multiline queries, and execute every
generated statement in a verified read-only `READ COMMITTED` transaction.

#### Scenario: Query execution
- **WHEN** the user enters a supported 1C query terminated by `;`
- **THEN** the CLI displays column labels, rows, and row count and then prompts
  for the next command

#### Scenario: Recoverable error
- **WHEN** compilation or PostgreSQL execution fails
- **THEN** the REPL prints the error, rolls back the statement transaction, and
  remains available

#### Scenario: Session lifecycle
- **WHEN** input reaches EOF or the user enters `\q`
- **THEN** the CLI closes the PostgreSQL connection and exits successfully

### Requirement: Provide metadata discovery commands
The REPL SHALL implement `\dt`, `\di`, `\d <metadata-name>`, `\refresh`,
`\help`, and `\q` using the resolved metadata snapshot.

#### Scenario: List tables
- **WHEN** the user enters `\dt`
- **THEN** the REPL lists logical kind/name, GUID, canonical physical table,
  SchemaStorage status, and live-catalog status

#### Scenario: List indexes
- **WHEN** the user enters `\di`
- **THEN** the REPL lists owning logical metadata, declared index, live index,
  normalized logical key, and match status

#### Scenario: Describe metadata
- **WHEN** the user enters `\d <qualified-or-unique-name>`
- **THEN** the REPL displays the object identity followed by its logical
  attributes, physical members/types, and declared/live indexes
