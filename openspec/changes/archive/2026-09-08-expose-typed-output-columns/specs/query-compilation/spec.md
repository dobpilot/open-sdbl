## ADDED Requirements

### Requirement: Expose structured output column kinds
Every compiled query SHALL describe each output column with its emitted label
and a structured `ColumnKind`: a reference with resolved target object IDs and
a runtime-typed flag, binary with optional length, string with optional length,
number with optional precision and scale, boolean, date-time, UUID, the `NULL`
literal, or an unknown catalog type carrying its raw type name. Kinds SHALL be
derived from the resolved live catalog and SchemaStorage without database
round trips, and every physical member of a queryable field SHALL expose the
same kind.

#### Scenario: Numeric catalog column
- **WHEN** a projected column is declared as `numeric(10,2)` on PostgreSQL or
  MSSQL
- **THEN** the compiled column kind is a number with precision 10 and scale 2

#### Scenario: Reference field
- **WHEN** a projected field is a SchemaStorage reference to one catalog
- **THEN** the compiled column kind is a reference whose targets contain that
  catalog's object ID and whose runtime-typed flag is false

#### Scenario: Unknown catalog type
- **WHEN** a projected column has a catalog type the compiler does not
  classify
- **THEN** the compiled column kind is unknown and carries the raw type name

### Requirement: Emit native-typed projections
Generated SQL SHALL project physical columns, scalar expressions, and
aggregates in their native database types without converting them to text.
The only conversions permitted are the MSSQL `_YearOffset` correction that
returns logical dates for date columns and a text cast for PostgreSQL
`mchar`/`mvarchar` columns of the 1C extension. Presentation functions MAY
still convert their arguments to text because their result is a string.

#### Scenario: Native reference projection
- **WHEN** a query projects a catalog `Ссылка`
- **THEN** generated SQL selects the physical reference column without a hex
  or text conversion

#### Scenario: MSSQL date with year offset
- **WHEN** a date column is projected for MSSQL with a non-zero year offset
- **THEN** generated SQL wraps it in `DATEADD(year, -offset, …)` and emits no
  `CONVERT`

#### Scenario: PostgreSQL 1C string type
- **WHEN** a `mvarchar` column is projected for PostgreSQL
- **THEN** generated SQL casts it to `text`

### Requirement: Project every reference as one column
A reference field SHALL occupy exactly one output column. Without an `RTRef`
member the column SHALL be the 16-byte `RRRef` value. With an `RTRef` member
the column SHALL be the binary concatenation of the 4-byte big-endian
`RTRef` and the 16-byte `RRRef`, and its kind SHALL be marked runtime-typed.
Other members of a compound field SHALL remain separate columns.

#### Scenario: Multi-type reference projection
- **WHEN** a query projects a field whose physical members are `_RTRef` and
  `_RRRef`
- **THEN** generated SQL emits one column concatenating both members and the
  compiled column kind is a runtime-typed reference

#### Scenario: Compound value members
- **WHEN** a compound field also stores `_S` and `_N` members
- **THEN** those members are projected as separate string and number columns

### Requirement: Diagnose UNION kind mismatches
When UNION branches project different column kinds at the same position, the
compiler SHALL fail with an unsupported-feature diagnostic positioned at the
union token before execution. The `NULL` literal and unknown catalog types
SHALL be compatible with every kind, and parameters such as length or
precision SHALL NOT participate in the comparison.

#### Scenario: Reference joined with string
- **WHEN** the first branch projects a reference and the second projects a
  string in the same position
- **THEN** compilation fails with an unsupported-feature diagnostic at the
  union keyword

#### Scenario: NULL branch
- **WHEN** one branch projects `NULL` where the other projects a number
- **THEN** compilation succeeds and the column kind is number

## MODIFIED Requirements

### Requirement: Emit unique result column labels
Generated result column labels SHALL be unique within a statement and SHALL
respect the target dialect's identifier length limit, truncating on a UTF-8
character boundary. PostgreSQL limits are measured in UTF-8 bytes and MSSQL
limits in UTF-16 code units. The compiled query's column metadata SHALL match
the labels actually emitted and SHALL pair every label with its column kind.

#### Scenario: Long colliding aliases
- **WHEN** two projection aliases exceed the dialect identifier limit and
  share a truncated prefix
- **THEN** the generated labels remain distinct and the compiled column list
  reports the emitted labels together with their kinds
