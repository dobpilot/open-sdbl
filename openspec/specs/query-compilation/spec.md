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
A nested statement SHALL allocate its own label set, so the same alias MAY
appear in a nested statement and in the statement that projects it.

#### Scenario: Long colliding aliases
- **WHEN** two projection aliases exceed the dialect identifier limit and
  share a truncated prefix
- **THEN** the generated labels remain distinct and the compiled column list
  reports the emitted labels together with their kinds

#### Scenario: Alias reused across nesting levels
- **WHEN** a nested source projects `Сумма` and the outer query projects the
  derived column under the same alias
- **THEN** both statements emit the label and the compiled column list
  reports it once for the outer statement

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
literal, the `НЕОПРЕДЕЛЕНО` literal, a type value, or an unknown catalog type
carrying its raw type name. Kinds SHALL be derived from the resolved live
catalog and SchemaStorage without database round trips, and every physical
member of a queryable field SHALL expose the same kind. A type value SHALL be
encoded as five bytes, the platform's `_TYPE` tag followed by the big-endian
`RTRef` table number, and the public `TypeValue` codec SHALL decode and encode
that representation and name the type through a snapshot.

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

#### Scenario: Type value column
- **WHEN** a projected column is `ТИПЗНАЧЕНИЯ(Т.Объект)`
- **THEN** the compiled column kind is `Type`, and `TypeValue::decode` turns
  the five bytes `0x08` + `RTRef` into the referenced object

### Requirement: Emit native-typed projections
Generated SQL SHALL project physical columns, scalar expressions, and
aggregates in their native database types without converting them to text.
The only conversions permitted are the MSSQL `_YearOffset` correction that
returns logical dates for date columns and a text cast for PostgreSQL
`mchar`/`mvarchar` values of the 1C extension, whose binary wire format is
undocumented. That cast SHALL apply both to a projected column of such a
type and to a projected computed expression of kind `String`, whose
operands may carry the extension type, in nested statements as well as in
the outer statement; an expression already rendered with a trailing text
cast SHALL NOT be cast again. Presentation functions MAY still convert
their arguments to text because their result is a string.

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

#### Scenario: PostgreSQL character expression
- **WHEN** a query projects `ЕСТЬNULL(Т.Наименование, "нет")`,
  `ВЫБОР … ТОГДА Т.Наименование … КОНЕЦ`, or `МАКСИМУМ(Т.Наименование)`
  over such a column
- **THEN** generated SQL casts that expression to `text`, and the driver
  decodes the value

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
precision SHALL NOT participate in the comparison. Reference columns whose
branches differ in target or width SHALL be widened to one runtime-typed
payload column whose targets are the union of the branch targets, so every
branch emits the same byte width.

#### Scenario: Reference joined with string
- **WHEN** the first branch projects a reference and the second projects a
  string in the same position
- **THEN** compilation fails with an unsupported-feature diagnostic at the
  union keyword

#### Scenario: NULL branch
- **WHEN** one branch projects `NULL` where the other projects a number
- **THEN** compilation succeeds and the column kind is number

#### Scenario: Fixed and runtime-typed reference branches
- **WHEN** one branch projects a catalog `Ссылка` and the other projects a
  runtime-typed `Регистратор`
- **THEN** the catalog branch is rendered as `RTRef ‖ RRRef` with the
  catalog's type number and the merged column kind is a runtime-typed
  reference containing the catalog among its targets

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

### Requirement: Diagnose parameter binding failures
The compiler SHALL report a `Parameter` diagnostic kind when a referenced
parameter has no supplied value, when a supplied parameter is never
referenced by the source, when a list value appears outside an `В`/`IN`
operand, or when a list contains a nested list. Missing-value and misplaced
diagnostics SHALL be positioned at the offending token; the unused-parameter
diagnostic MAY be unpositioned. Preparation SHALL NOT require parameter
values.

#### Scenario: Missing value
- **WHEN** the source references `&Период` and the options carry no
  parameter of that name in any letter case
- **THEN** compilation fails with a `Parameter` diagnostic located at the
  `&Период` token

#### Scenario: Unused value
- **WHEN** the options carry a parameter the source never references
- **THEN** compilation fails with a `Parameter` diagnostic naming it

#### Scenario: Preparation without values
- **WHEN** a parameterized source is prepared
- **THEN** preparation succeeds and the presentation request is collected

### Requirement: Diagnose temporary-table failures
`QueryDiagnosticKind` SHALL include a `TemporaryTable` variant reported
with the offending token for an unknown, hidden, or duplicate temporary
table name, an `ДОБАВИТЬ` structure mismatch, an index field outside the
selection list, a `TempTablesManager` bound to another dialect or holding
its maximum of 256 definitions, and a batch that returns no rows when
compiled through an entry point that must return SQL. Callers matching the
non-exhaustive enum SHALL keep their fallback arm.

#### Scenario: Unknown temporary table
- **WHEN** a statement reads `ИЗ ВТ` and no visible definition named `ВТ`
  exists
- **THEN** the diagnostic kind is `TemporaryTable` and its position is the
  `ВТ` token

#### Scenario: Duplicate definition
- **WHEN** a batch places `ВТ` twice without dropping it in between
- **THEN** the diagnostic kind is `TemporaryTable` at the second name token

### Requirement: Accept the allowed keyword in statement position
The parser SHALL accept `РАЗРЕШЕННЫЕ`/`ALLOWED` directly after `ВЫБРАТЬ`
of the first top-level branch of a statement, before
`РАЗЛИЧНЫЕ`/`DISTINCT` and `ПЕРВЫЕ`/`TOP`, and SHALL apply it to every
nested query and union branch of that statement. The keyword in a later
union branch or inside a nested query SHALL be a `Syntax` diagnostic at the
keyword token. Statements of a batch SHALL be independent: a statement
without the keyword reads its sources unrestricted even when another
statement of the batch carries it.

#### Scenario: Keyword order
- **WHEN** a statement starts with `ВЫБРАТЬ РАЗРЕШЕННЫЕ РАЗЛИЧНЫЕ ПЕРВЫЕ 10`
- **THEN** the statement compiles with distinct rows and a row limit

#### Scenario: Keyword in a nested query
- **WHEN** a source is `(ВЫБРАТЬ РАЗРЕШЕННЫЕ … ) КАК В`
- **THEN** compilation fails with a `Syntax` diagnostic at the keyword

### Requirement: Request access restrictions for allowed statements
Preparation SHALL collect a `RestrictionRequest` listing every metadata
object, together with the tabular-section name when the source is a
tabular section, that statements carrying the keyword read through
`ИЗ`, joins, nested queries, `В (ВЫБРАТЬ …)` subqueries, and virtual
tables, deduplicated and in stable order. Temporary tables and derived
sources SHALL NOT appear. The request SHALL be empty for a batch without
the keyword.

#### Scenario: Joined sources
- **WHEN** `ВЫБРАТЬ РАЗРЕШЕННЫЕ …` joins a catalog with a document tabular
  section
- **THEN** the request lists the catalog and the document with its section
  name, once each

#### Scenario: Statement without the keyword
- **WHEN** a batch reads tables only through statements without the keyword
- **THEN** the request is empty

### Requirement: Apply access restrictions as source-level conjunctions
`CompileOptions::restrictions` SHALL supply `AccessRestriction` values, each
naming a target object, an optional tabular-section name, and an SDBL
condition. A plain source read by a statement with the keyword whose target
has a restriction SHALL render as a derived table projecting every physical
column of the source, aliased `__restricted`, with the condition compiled
as if written in `ГДЕ` over `ИЗ <target>`: fields unqualified, one-hop
dereferences rendered as `LEFT JOIN`s inside the derived table, nested
`В (ВЫБРАТЬ …)` allowed, parameters resolved against session parameters
only. Sources reached by a restriction's own nested queries SHALL NOT be
restricted. The same wrapper SHALL be used for `ИЗ` and every join kind.
Slice, balance, and turnover sources SHALL conjoin the compiled condition
with their virtual-table predicate and SHALL accept only the direct fields
their own condition accepts. A target without a restriction, and every
statement without the keyword, SHALL generate the same SQL as before;
restrictions SHALL NOT be merged with the query's own predicates.

#### Scenario: Restricted catalog in a join
- **WHEN** a restricted catalog is the right side of a `ЛЕВОЕ СОЕДИНЕНИЕ`
- **THEN** the join reads `(SELECT … FROM <table> AS "__restricted" WHERE
  <condition>) AS <alias>` and the query's `ГДЕ` stays unchanged

#### Scenario: Dereference inside a restriction
- **WHEN** the restriction text is `Владелец.Ответственный = &Пользователь`
- **THEN** the derived table left-joins the owner target and compares its
  attribute with the session parameter value

#### Scenario: Restricted balance source
- **WHEN** an accumulation-register `Остатки` source has a restriction on a
  dimension
- **THEN** the condition is conjoined to the totals and movement predicates

#### Scenario: Unrestricted target
- **WHEN** the application supplies no restriction for a requested target
- **THEN** the generated SQL is identical to the SQL without the keyword

### Requirement: Resolve session parameters
`CompileOptions::session` SHALL supply `SessionParameters`, a set of named
values unique by case-insensitive name. `&Имя` in a query SHALL resolve to
the query parameter of that name when supplied and to the session
parameter otherwise; a session parameter that no query or restriction
references SHALL NOT be an error. Restriction text SHALL see session
parameters only.

#### Scenario: Session fallback
- **WHEN** the query references `&ТекущийПользователь` and only the session
  parameters carry it
- **THEN** the session value is inlined

#### Scenario: Query value wins
- **WHEN** both the query parameters and the session parameters carry `&Орг`
- **THEN** the query value is inlined

### Requirement: Diagnose restriction failures
The compiler SHALL report a `Restriction` diagnostic kind when a
restriction's text fails to lex, parse, resolve, or generate, positioned
inside the restriction text and naming the target in the message; when a
supplied restriction matches no target read by a statement with the
keyword; and when two restrictions name the same target. Preparation SHALL
NOT require restrictions.

#### Scenario: Unknown field in the restriction
- **WHEN** the restriction text names a field the target does not have
- **THEN** compilation fails with a `Restriction` diagnostic whose position
  is the field token inside the restriction text

#### Scenario: Unused restriction
- **WHEN** a restriction names an object no statement with the keyword
  reads
- **THEN** compilation fails with a `Restriction` diagnostic naming it

### Requirement: Resolve data separator values from session parameters
For every separator field of the snapshot, statement compilation SHALL
determine one value: when the separator's use-flag session parameter is
present with the value `ЛОЖЬ`, the separator is disabled for the
statement; otherwise the value is the session parameter bound as the
separator value, or the session parameter named like the common attribute
when Config binds none. When neither is present, an
`IndependentAndShared` separator SHALL use the empty value of its kind
(`0`, `""`, `ЛОЖЬ`, the empty date) rendered as the dialect's typed
literal, and an `Independent` separator SHALL fail with a `Parameter`
diagnostic at the first source token that reads a table declaring its
column, naming the separator and the expected session parameter. Query
parameters SHALL NOT supply separator values, and preparation (which runs
without values) SHALL NOT fail on them.

#### Scenario: Session value
- **WHEN** `ОбластьДанныхЗначение` is set to `7` in the session parameters
- **THEN** every separated table read by the statement is filtered by
  `"_Fld<N>" = 7`

#### Scenario: Shared default
- **WHEN** no session parameter is set and the separator is
  `IndependentAndShared`
- **THEN** the tables are filtered by the numeric literal `0`

#### Scenario: Disabled separator
- **WHEN** `ОбластьДанныхИспользование` is set to `ЛОЖЬ`
- **THEN** the statement generates no separator predicate

#### Scenario: Independent separator without a value
- **WHEN** no session parameter is set and the separator is `Independent`
- **THEN** compilation fails with a `Parameter` diagnostic that names the
  separator and the session parameter it expects

### Requirement: Filter every separated table by its separator
The compiler SHALL conjoin `<alias>."_Fld<N>" = <value>` for each
separator column declared by the physical table it reads: main tables,
tabular sections, each extension `UNION ALL` branch that declares the
column, change-registration and calculation-kind tables, dereference and
presentation joins, the base reads of slices, balances, and turnovers,
constants, sources inside nested queries and `В (ВЫБРАТЬ …)`, and sources
inside restriction bodies. A source introduced by `ВНУТРЕННЕЕ` or
`ЛЕВОЕ` SHALL carry its predicate in its own `ON`; the first source and a
source introduced by `ПРАВОЕ` SHALL carry it in the `ON` of the next
`ПРАВОЕ` join that null-extends them, or in `WHERE` when none follows;
dereference and presentation joins SHALL carry it in their `ON`. Each
direction of the `ПОЛНОЕ СОЕДИНЕНИЕ` emulation SHALL filter its
null-extended side in `ON` and its preserved side in `WHERE`. Temporary
tables and derived sources SHALL NOT be filtered.
A snapshot without separators, and a table without the column, SHALL
generate SQL byte-identical to a compilation without this requirement.

#### Scenario: Reference filter seeks the primary key
- **WHEN** `ВЫБРАТЬ … ИЗ Справочник.Номенклатура ГДЕ Ссылка = &Ссылка`
  compiles on a separated base
- **THEN** the `WHERE` reads `"_Fld<N>" = <value> AND "_IDRRef" = <id>`

#### Scenario: Left join and dereference
- **WHEN** a catalog is joined with `ЛЕВОЕ СОЕДИНЕНИЕ` and a field of the
  left side is dereferenced
- **THEN** the left side is filtered in `WHERE`, and the joined table and
  the dereference join each carry the predicate in their `ON`

#### Scenario: Full join
- **WHEN** two separated tables are joined with `ПОЛНОЕ СОЕДИНЕНИЕ`
- **THEN** both `LEFT JOIN` directions of the emulation filter the joined
  side in `ON` and the base side in `WHERE`, so no row of another area
  survives as an unmatched row

#### Scenario: Extension branch without the column
- **WHEN** an object's `X1` extension table does not declare the
  separator column
- **THEN** only the base branch of the `UNION ALL` carries the predicate

#### Scenario: Base without separators
- **WHEN** the snapshot has no separator fields
- **THEN** the generated SQL is unchanged

### Requirement: Compile the constants table source
The compiler SHALL accept `Константы`/`Constants` as a source whose
fields are the names of every constant with a live `_Const<N>` table,
typed as the single-constant source types them, with no standard fields.
The source SHALL render as a derived table reading only the constants the
statement references (`*` references all): a `UNION ALL` of one `SELECT`
per constant projecting its physical columns and `CAST(NULL AS <catalog
type>)` for the columns of the other constants, aggregated with `MAX` per
column and without `GROUP BY`, so the result is exactly one row; a
statement referencing no constant reads a one-row stand-in. Separator predicates SHALL
apply inside every branch; a referenced constant whose separator is
disabled for the statement SHALL be an `UnsupportedFeature` diagnostic at
the source naming the constant. `Константа.Имя` SHALL keep its rendering.

#### Scenario: Two constants
- **WHEN** `ВЫБРАТЬ К.А, К.Б ИЗ Константы КАК К` compiles
- **THEN** the source is `(SELECT MAX(u."_Fld<A>") …, MAX(u."_Fld<B>") …
  FROM (SELECT t."_Fld<A>", CAST(NULL AS <type of B>) FROM "_Const<A>" AS
  t UNION ALL SELECT CAST(NULL AS <type of A>), t."_Fld<B>" FROM
  "_Const<B>" AS t) AS u) AS "К"` and constants the statement does not
  reference are absent

#### Scenario: Unwritten constant
- **WHEN** a referenced constant's table has no rows
- **THEN** the query still returns one row with `NULL` in that column

#### Scenario: Reference constant dereference
- **WHEN** `ВЫБРАТЬ Константы.Организация.Наименование ИЗ Константы`
  compiles
- **THEN** the derived table projects the `RRef` and `_TYPE` columns and
  the dereference renders as a `LEFT JOIN` on them

#### Scenario: Disabled separator
- **WHEN** a referenced constant table declares a separator column and the
  separator is disabled for the statement
- **THEN** compilation fails with an `UnsupportedFeature` diagnostic naming
  the constant

### Requirement: Compare a composite field with a value
A field stored in several physical members SHALL be comparable with a
value by `=`, `<>` and `В (…)`. The rendered predicate SHALL test the
`_TYPE` discriminator against the type tag of the value and the member
that carries a value of that type, which is how the platform renders its
own comparison. A reference value SHALL also test the `RTRef` member, and
where the field admits a single reference type and stores no `RTRef`
member the comparison SHALL synthesize it from the discriminator. A value
whose type the field cannot hold SHALL compare false instead of failing,
and an unbound parameter SHALL render as a comparison with `NULL`.

#### Scenario: Reference value
- **WHEN** `ГДЕ Т.Объект = &Ссылка` compares a composite field with a
  bound catalog reference
- **THEN** only rows holding that reference answer, as on the platform

#### Scenario: Value of another stored type
- **WHEN** the same field is compared with a string
- **THEN** only rows holding that string answer

#### Scenario: List of values
- **WHEN** `ГДЕ Т.Объект В (НЕОПРЕДЕЛЕНО, ЗНАЧЕНИЕ(…), "текст")` is
  compiled
- **THEN** the predicate is the disjunction of the member comparisons of
  the listed values

#### Scenario: Value of an impossible type
- **WHEN** a composite field that holds no number is compared with one
- **THEN** the comparison answers no rows instead of reporting a
  diagnostic

### Requirement: Precedence of the negation operator
`НЕ` / `NOT` SHALL bind looser than every comparison and tighter than `И`,
so that it negates the comparison, `ПОДОБНО`, `В`, `В ИЕРАРХИИ`, `ССЫЛКА`
or `ЕСТЬ NULL` written to its right, and groups before a conjunction. The
unary sign operators keep binding to the operand next to them.

#### Scenario: Negated comparison
- **WHEN** `ГДЕ НЕ Т.Цена = 10` is compiled
- **THEN** the predicate negates the comparison, answering every row whose
  price differs from ten, as the platform does

#### Scenario: Negation before a conjunction
- **WHEN** `ГДЕ НЕ Т.Цена > 10 И Т.Цена < 30` is compiled
- **THEN** the negation covers only the first comparison

### Requirement: Joins nested inside a join source
A join source SHALL accept further joins written before its own `ПО`, so
that the conditions close in reverse order, and the group SHALL compile as
the flat chain of the same joins with the outer one first. Where that
rewrite would change the result — an outer `ЛЕВОЕ`, `ПРАВОЕ` or `ПОЛНОЕ`
join containing a join that is not `ЛЕВОЕ` — the compiler SHALL report an
unsupported-feature diagnostic instead.

#### Scenario: Group of left joins
- **WHEN** `A ЛЕВОЕ СОЕДИНЕНИЕ B ЛЕВОЕ СОЕДИНЕНИЕ C ПО <B‑C> ПО <A‑B>` is
  compiled
- **THEN** the SQL joins B on the second condition and C on the first,
  answering what the platform answers

#### Scenario: Inner join inside a left join
- **WHEN** the nested join is `ВНУТРЕННЕЕ`
- **THEN** the compiler reports that the grouping is not supported

### Requirement: Nesting budget fits a small stack
The parser SHALL report `TooDeep` before a nested expression can exhaust
the stack of a small thread, and the budget SHALL be 64 nesting levels.

#### Scenario: Deeply nested functions
- **WHEN** a query nests a date function far beyond the budget
- **THEN** the compiler reports that the nesting depth exceeds the limit
  of 64 instead of aborting the process

### Requirement: Simple form of ВЫБОР
`ВЫБОР <выражение> КОГДА <значение> ТОГДА …` SHALL compile every
alternative as the comparison of the subject with the value of that
alternative, using the same rules as a comparison written in `ГДЕ`, so
that references, composite fields and type values answer alike. A subject
that is `NULL` SHALL match no alternative.

#### Scenario: Alternatives of a simple ВЫБОР
- **WHEN** `ВЫБОР Т.Цена КОГДА 10 ТОГДА "десять" ИНАЧЕ "прочее" КОНЕЦ` is
  compiled
- **THEN** rows whose price is ten answer the first value and every other
  row answers the alternative, as on the platform

#### Scenario: Type value as the subject
- **WHEN** the subject is `ТИПЗНАЧЕНИЯ(поле)` and an alternative is
  `ТИП(Справочник.X)`
- **THEN** the alternative matches the rows holding a reference to that
  catalog

### Requirement: Tabular sections of every kind that has them
A tabular-section source SHALL be accepted for every object kind that
stores tabular sections — catalogs, documents, charts of characteristic
types, charts of accounts, charts of calculation types, business
processes, tasks and exchange plans — and refused for the kinds that have
none.

#### Scenario: Tabular section of a business process
- **WHEN** `ИЗ БизнесПроцесс.X.ТабличнаяЧасть КАК Т` is compiled
- **THEN** the source resolves to the tabular-section table of that
  business process

### Requirement: Metadata names spelled like keywords
The kind, object and value names of `ЗНАЧЕНИЕ(…)` SHALL accept a name
that the lexer reads as a keyword, because nothing but a name may appear
in those positions.

#### Scenario: Enumeration value named like a keyword
- **WHEN** `ЗНАЧЕНИЕ(Перечисление.X.НеОпределено)` is compiled
- **THEN** the value resolves like any other predefined value

### Requirement: Read a document journal
`ЖурналДокументов.<Имя>` / `DocumentJournal.<Name>` SHALL be a source. Its
standard fields SHALL be `Ссылка` — the reference of the registered
document, stored as one column for a journal of a single document kind and
as the `RTRef ‖ RRRef` pair otherwise — together with `Тип`, which answers
the type value of that reference, and `Дата`, `Номер`, `ПометкаУдаления`
and `Проведен`. The journal's own columns SHALL answer under their
metadata names.

#### Scenario: Projection of a journal
- **WHEN** `ВЫБРАТЬ Ж.Ссылка, Ж.Дата, Ж.Номер, Ж.Клиент ИЗ
  ЖурналДокументов.ЖурналПродаж КАК Ж` is compiled
- **THEN** each field reads its column of the journal table

#### Scenario: Type of the registered document
- **WHEN** `Ж.Тип` is read
- **THEN** it answers the type value of the journal's reference, which
  compares with `ТИП(Документ.X)`

#### Scenario: Dereference through the journal reference
- **WHEN** `Ж.Ссылка.Дата` is read from a journal of a single document kind
- **THEN** the document is joined and its field answers

### Requirement: Standard fields of business processes and tasks
A business process SHALL expose `Completed` / `Завершен`, `Started` /
`Стартован` and `HeadTask` / `ВедущаяЗадача`; a task SHALL expose `Name` /
`Наименование`, `Executed` / `Выполнена`, `BusinessProcess` /
`БизнесПроцесс` and `Point` / `ТочкаМаршрута`, next to the reference,
date, number and deletion mark they already carry.

#### Scenario: Task list
- **WHEN** `ВЫБРАТЬ З.Наименование, З.Выполнена, З.БизнесПроцесс ИЗ
  Задача.X КАК З` is compiled
- **THEN** each name reads its column of the task table

#### Scenario: Business process state
- **WHEN** `ГДЕ Б.Завершен` filters a business process
- **THEN** the predicate reads the `Completed` column

### Requirement: Nested tabular-section projection is named
A field path followed by `.(…)` or `.*` asks for a tabular section as a
nested result inside one column. The compiler SHALL report an
unsupported-feature diagnostic that names that construct, because one SQL
statement returns no nested result.

#### Scenario: Nested column list
- **WHEN** `ВЫБРАТЬ Т.Состав.(Ссылка, НомерСтроки) ИЗ Справочник.X КАК Т`
  is compiled
- **THEN** the diagnostic says the nested tabular-section result is not
  supported and points at the construct

#### Scenario: Nested wildcard
- **WHEN** the projection is `Т.Состав.*`
- **THEN** the same diagnostic is reported

### Requirement: Read a filter criterion
`КритерийОтбора.<Имя>(<значение>)` / `FilterCriterion` SHALL be a source
that answers every object whose field listed in the criterion's content
holds the value. It SHALL compile as one `SELECT` per content field united
by `UNION ALL`, each projecting the found object as the `RTRef ‖ RRRef`
payload of the single field `Ссылка`, so that the field dereferences,
groups and joins like a reference of a derived source. A reference value
SHALL be compared by its 16-byte identifier. A criterion whose content
reaches no live field SHALL be refused.

#### Scenario: Objects found by a criterion
- **WHEN** `ВЫБРАТЬ К.Ссылка ИЗ КритерийОтбора.X(&Значение) КАК К` is
  compiled
- **THEN** the relation unites one selection per content field, each
  filtered by the value

#### Scenario: Dereference of the found object
- **WHEN** `К.Ссылка.Наименование` is read
- **THEN** each target is joined under its own type guard, as for any
  payload reference

### Requirement: Correlated subquery of a predicate
A subquery written in a predicate SHALL resolve the qualifiers of the
enclosing statement's sources and render them as the outer alias, so that
it filters by the row being tested. An unqualified name SHALL keep
resolving against the subquery's own sources only, and a derived source
SHALL keep refusing an outer qualifier, because SQL evaluates it before
the outer row exists.

#### Scenario: Existence check
- **WHEN** `ГДЕ Т.Код В (ВЫБРАТЬ Л.Код ИЗ Справочник.X КАК Л ГДЕ
  Л.Дата = Т.Дата)` is compiled
- **THEN** the subquery compares with the outer alias, as on the platform

#### Scenario: Derived source stays uncorrelated
- **WHEN** an outer qualifier is used inside `ИЗ (ВЫБРАТЬ …) КАК Д`
- **THEN** the compiler reports an unknown field

### Requirement: Order of an enumeration value
`Порядок` / `Order` SHALL name the `EnumOrder` column, which the platform
answers as the zero-based declaration order of the value.

#### Scenario: Ordering by the declaration order
- **WHEN** `ВЫБРАТЬ П.Порядок ИЗ Перечисление.X КАК П` is compiled
- **THEN** the column answers 0, 1, 2 … in declaration order

### Requirement: Presentation of a source
`Источник.Представление` SHALL present the reference of that source,
deferred to the application exactly as `ПРЕДСТАВЛЕНИЕССЫЛКИ` of its
reference field is. A source whose rows carry no reference SHALL be
refused with a diagnostic naming it.

#### Scenario: Presentation of a catalog source
- **WHEN** `ВЫБРАТЬ Т.Представление ИЗ Справочник.X КАК Т` is compiled
- **THEN** the column is requested as a presentation of that catalog's
  reference

### Requirement: Alternatives of different types
A projected `ВЫБОР` or `ЕСТЬNULL` whose alternatives differ in type SHALL
be rendered as the members of a composite value, the way the platform
stores one: the `_TYPE` discriminator naming the type of each row, one
column per type present among the alternatives, and the reference payload
when a branch carries a reference. Each branch SHALL write its value into
its own member and the zero of the type into the others. Each member
SHALL carry the output label of the projection with the suffix a projected
composite field uses.

#### Scenario: String and reference alternatives
- **WHEN** `ВЫБОР КОГДА … ТОГДА "дорого" ИНАЧЕ Т.Клиент КОНЕЦ КАК Смесь`
  is projected
- **THEN** the result carries `Смесь` with the reference payload,
  `Смесь_S` with the string and `Смесь_TYPE` with the discriminator

#### Scenario: Alternatives of two primitive types
- **WHEN** the alternatives are a number and a string
- **THEN** only the number, string and discriminator members are projected

### Requirement: An alias hides the object name
A source that declares an alias SHALL be addressed by that alias alone.
The object name SHALL qualify only a source written without an alias,
which the platform does not accept at all and the compiler keeps as a
convenience.

#### Scenario: The same catalog read twice
- **WHEN** one statement reads a catalog under an alias and a nested
  statement reads it under another
- **THEN** each qualifier names exactly one source

### Requirement: A tabular section named as a field
A name that resolves to no field but names a tabular section of the source
SHALL report that a tabular section as a nested result of the selection is
not supported.

#### Scenario: Tabular section in the selection list
- **WHEN** `ВЫБРАТЬ Т.Состав ИЗ Справочник.X КАК Т` is compiled
- **THEN** the diagnostic names the nested result instead of an unknown
  field

### Requirement: Periodicity of ОстаткиИОбороты
`ОстаткиИОбороты` SHALL accept a calendar periodicity, group the
movements of the interval into those periods and expose `Период`, which is
what the platform answers where no balance column is read. A period
completion method SHALL be accepted only together with a periodicity, as
both methods answer the same rows there. A balance column of a periodic
table SHALL be refused, because its value is a running sum over the
periods before it, which the platform accumulates outside SQL.

#### Scenario: Turnovers by month
- **WHEN** `ВЫБРАТЬ О.Период, О.КоличествоОборот ИЗ
  РегистрНакопления.X.ОстаткиИОбороты(, , Месяц, ) КАК О` is compiled
- **THEN** the relation groups the movements by month and answers one row
  per month with movements

#### Scenario: Balance of a periodic table
- **WHEN** the statement reads `КоличествоНачальныйОстаток` of a periodic
  table
- **THEN** the compiler reports that a periodic table answers no balance
  column

### Requirement: Condition without a source
A statement without a source SHALL accept `ГДЕ` and render it as a
`WHERE` clause with no `FROM`, which is what the platform answers: a false
condition yields no row and a true one yields the single row of the
projection.

#### Scenario: Constant row filtered away
- **WHEN** `ВЫБРАТЬ 1 КАК Т ГДЕ ЛОЖЬ` is compiled and executed
- **THEN** no row is answered

### Requirement: Value type of a chart of characteristic types
`ТипЗначения` / `ValueType` SHALL name the `Type` column of a chart of
characteristic types.

#### Scenario: Projecting the value type
- **WHEN** `ВЫБРАТЬ П.ТипЗначения ИЗ ПланВидовХарактеристик.X КАК П` is
  compiled
- **THEN** the column reads the chart's `Type` column

### Requirement: Narrow an expression to a metadata type
`ВЫРАЗИТЬ(<выражение> КАК <Вид>.<Объект>)` SHALL narrow a computed value:
a reference of that type keeps its value, a runtime-typed payload is
narrowed by its type prefix and answers `NULL` for another type, and a
value that is `NULL` whatever its type narrows to `NULL` of the named
type. Every other kind SHALL be refused, as the platform refuses it.

#### Scenario: Alternatives narrowed to their type
- **WHEN** `ВЫРАЗИТЬ(ВЫБОР … ТОГДА Т.Клиент ИНАЧЕ ЗНАЧЕНИЕ(…) КОНЕЦ КАК
  Справочник.Клиенты)` is compiled
- **THEN** the value is kept as a reference of that catalog

#### Scenario: Value that cannot hold the type
- **WHEN** a number is narrowed to a catalog
- **THEN** the compiler refuses it, as the platform does
