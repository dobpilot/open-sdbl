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
the labels actually emitted and SHALL pair every label with its column kind.

#### Scenario: Long colliding aliases
- **WHEN** two projection aliases exceed the dialect identifier limit and
  share a truncated prefix
- **THEN** the generated labels remain distinct and the compiled column list
  reports the emitted labels together with their kinds

### Requirement: Validate MSSQL year offsets at construction
The MSSQL backend SHALL validate its year offset when constructed and SHALL
reject values outside the supported range instead of overflowing during
compilation.

#### Scenario: Extreme offset
- **WHEN** an application constructs an MSSQL backend with an extreme
  integer offset
- **THEN** construction fails with an error and no later compilation can
  overflow date arithmetic

### Requirement: Bind prepared queries to their snapshot
A prepared query SHALL record the identity of the metadata snapshot it
was prepared against and SHALL refuse to compile with a snapshot whose
identity differs, reporting a machine-readable diagnostic instead of
resolving presentation plans against unrelated metadata.

#### Scenario: Compiling with a different snapshot
- **WHEN** a query prepared against one snapshot is compiled with a
  snapshot resolved from different metadata
- **THEN** compilation fails with a snapshot-mismatch diagnostic kind

#### Scenario: Compiling with the original snapshot
- **WHEN** the same snapshot used for preparation is supplied to compile
- **THEN** compilation proceeds normally

### Requirement: Bound total compilation work
Compilation SHALL enforce an overall work budget covering union
branches, projections, and reference resolution, so that a source text
within the parser's syntactic limits cannot consume unbounded CPU
through repetition. Charges SHALL reflect work proportional to the query
and the projected sources; catalog lookups by table name SHALL be indexed
so that the size of the information base does not consume the budget.

#### Scenario: Pathological repetition
- **WHEN** a query multiplies many union branches over sources whose
  field resolution is expensive
- **THEN** compilation either completes promptly or fails fast with a
  typed work-budget diagnostic

#### Scenario: Large information base
- **WHEN** a snapshot contains tens of thousands of live and SchemaStorage
  tables and a query joins two sources with dereferenced presentations
- **THEN** compilation succeeds within the work budget

### Requirement: Compile change-registration sources
The compiler SHALL accept the bilingual change-registration spelling
(`<ВидОбъекта>.<X>.Изменения` / `<ObjectKind>.<X>.Changes`) on a registered
object as a FROM source and generate SQL over that object's
change-registration table, projecting the exchange-plan node reference,
message number, and the object's key columns, for both supported
dialects.

#### Scenario: Selecting registered changes
- **WHEN** a query selects the changes of an object registered with an
  exchange plan
- **THEN** both dialects produce SQL over that object's
  change-registration table with node and key columns resolvable by
  name

### Requirement: Compile calculation-kind dependency sources
The compiler SHALL expose leading, base, and displaced calculation-kind
tables as tabular-section-like sources of their chart of calculation
kinds on both dialects.

#### Scenario: Leading calculation kinds
- **WHEN** a query selects from the leading-calculation-kinds table of a
  chart of calculation kinds
- **THEN** the generated SQL reads the dependency table joined to its
  owner keys

### Requirement: Compile extension-added attributes as ordinary fields
Attributes added by configuration extensions SHALL be usable wherever
base attributes are: projection, filtering, ordering, and dereference,
compiling to the extension's physical columns without dedicated syntax.

#### Scenario: Filtering by an extension attribute
- **WHEN** a query filters on an attribute that exists only in an
  extension
- **THEN** compilation succeeds on both dialects and references the
  extension table's column

### Requirement: Diagnose resolve-only service sources
Service tables resolved as metadata but without query support SHALL
produce a machine-readable diagnostic when used as a FROM source, not
silent failure or invalid SQL.

#### Scenario: Unsupported service source
- **WHEN** a query names a resolve-only service table as its source
- **THEN** compilation fails with a typed unsupported-feature diagnostic
  naming the table

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

### Requirement: Render SQL for the selected MSSQL dialect level
The MSSQL backend value SHALL carry a dialect level (`Sql2008` or `Sql2012`,
defaulting to `Sql2012`) that every compilation, preparation, and
presentation-lookup path honours. On `Sql2008` generated T-SQL SHALL use
only functions available on SQL Server 2008, emulating newer functions with
equivalent arithmetic, and SHALL produce the same logical values as on
`Sql2012`. Statements that never needed newer functions SHALL be identical
across levels.

#### Scenario: Beginning of period on SQL Server 2008
- **WHEN** `НАЧАЛОПЕРИОДА(Дата, МЕСЯЦ)` is compiled with the `Sql2008` level
- **THEN** generated SQL uses `DATEADD`/`DATEDIFF` from a `datetime2` base
  and contains no `DATETIME2FROMPARTS`

#### Scenario: Level parity
- **WHEN** a query without `НАЧАЛОПЕРИОДА` is compiled on both levels
- **THEN** the generated SQL is identical

#### Scenario: Default level
- **WHEN** an application constructs `MsSqlBackend::new(year_offset)` without
  choosing a level
- **THEN** the backend reports `Sql2012` and generates the same SQL as before
  levels existed

### Requirement: Emit PostgreSQL SQL portable to 9.0
Generated PostgreSQL SQL SHALL avoid constructs introduced after PostgreSQL
9.0 so that one stateless backend serves every supported server.

#### Scenario: Balance anchor aggregate
- **WHEN** an accumulation-register balance query is compiled
- **THEN** the anchor period uses `MAX(CASE WHEN … END)` rather than
  `FILTER (WHERE …)`
