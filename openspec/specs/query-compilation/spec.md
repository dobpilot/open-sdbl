# query-compilation Specification

## Purpose
Define bounded and deterministic compilation of 1C SDBL into PostgreSQL and
MSSQL SQL, with backend-correct output and machine-readable failures for
syntax, metadata resolution, and unsupported operations.

## Requirements

### Requirement: Bound query parsing work
The query compiler SHALL enforce a fixed nesting-depth limit while parsing
SDBL and SHALL report exceeding it as a positional diagnostic. No source
text, regardless of size or nesting, may abort the process.

#### Scenario: Deeply nested parentheses
- **WHEN** a query contains thousands of nested parentheses or chained unary
  operators
- **THEN** compilation returns a depth-limit diagnostic pointing at the
  source position where the limit was exceeded

### Requirement: Expose machine-readable diagnostic kinds
Every query diagnostic SHALL carry a machine-readable kind alongside its
message, and diagnostics wrapping lexical or metadata lookup failures SHALL
expose the underlying error through the standard error-source chain.

#### Scenario: Distinguishing failure classes
- **WHEN** compilation fails because a field is unknown and, separately,
  because a metadata object name is ambiguous
- **THEN** the two diagnostics expose distinct kinds without requiring
  message-text comparison

### Requirement: Report accurate diagnostic positions
Diagnostics raised while resolving metadata during compilation SHALL point
at the source token that triggered resolution rather than a fabricated
start-of-query position.

#### Scenario: Unknown metadata object
- **WHEN** the FROM clause names a metadata object that does not exist
- **THEN** the diagnostic's line and column locate that object name in the
  query source

#### Scenario: Unexpected end of query
- **WHEN** parsing fails because a required token is absent at end of input
- **THEN** the diagnostic offset, line, and column identify the actual end of
  the supplied source rather than the start of the query

### Requirement: Quote identifiers per SQL dialect
Generated SQL SHALL quote every identifier with the target dialect's
canonical quoting: double quotes with doubling for PostgreSQL and square
brackets with `]]` escaping for MSSQL, independent of session settings such
as `QUOTED_IDENTIFIER`.

#### Scenario: MSSQL identifier quoting
- **WHEN** a query is compiled for the MSSQL backend
- **THEN** every table, column, and alias identifier in the generated T-SQL
  uses bracket quoting

### Requirement: Reuse joins deterministically
Reference dereferencing and reference presentation SHALL share one join
deduplication key covering the source scope, source field, join target, and
reference type guard, so a join carrying a type guard is never silently
reused for an access requiring different guard semantics.

#### Scenario: Dereference and presentation of one multi-target field
- **WHEN** a query both dereferences and requests the presentation of the
  same multi-target reference field
- **THEN** the generated SQL joins each target with its own correctly
  guarded join and column references resolve against the matching join

### Requirement: Emit unique result column labels
Generated result column labels SHALL be unique within a statement and SHALL
respect the target dialect's identifier length limit, truncating on a UTF-8
character boundary. PostgreSQL limits are measured in UTF-8 bytes and MSSQL
limits in UTF-16 code units. The compiled query's column metadata SHALL match
the labels actually emitted.

#### Scenario: Long colliding aliases
- **WHEN** two projection aliases exceed the dialect identifier limit and
  share a truncated prefix
- **THEN** the generated labels remain distinct and the compiled column list
  reports the emitted labels

### Requirement: Validate MSSQL year offsets at construction
The MSSQL backend SHALL validate its year offset when constructed and SHALL
reject values outside the supported range instead of overflowing during
compilation.

#### Scenario: Extreme offset
- **WHEN** an application constructs an MSSQL backend with an extreme
  integer offset
- **THEN** construction fails with an error and no later compilation can
  overflow date arithmetic
