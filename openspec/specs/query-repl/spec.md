# query-repl Specification

## Purpose
Define safe compilation and interactive execution of a bounded read-only 1C
query subset against PostgreSQL, together with metadata discovery commands
backed by authoritative resolved 1C metadata.

## Requirements

### Requirement: Compile a bounded read-only 1C query subset
The `open-sdbl` library SHALL compile one or more
`ВЫБРАТЬ`/`SELECT` branches into one PostgreSQL SELECT statement using only
authoritative resolved metadata. Branches MAY be connected with
`ОБЪЕДИНИТЬ`/`UNION` or `ОБЪЕДИНИТЬ ВСЕ`/`UNION ALL`. An unjoined
branch SHALL support projection or `*`, one metadata source, an optional source
alias with or without `КАК`/`AS`, one-hop reference property paths,
`РАЗЛИЧНЫЕ`/`DISTINCT`, `ПЕРВЫЕ`/`TOP`, and basic `ГДЕ`/`WHERE`
expressions. A branch MAY instead chain one or more
`[ВНУТРЕННЕЕ] СОЕДИНЕНИЕ` / `[INNER] JOIN`, `ЛЕВОЕ [ВНЕШНЕЕ]
СОЕДИНЕНИЕ` / `LEFT [OUTER] JOIN`, and `ПРАВОЕ [ВНЕШНЕЕ] СОЕДИНЕНИЕ` /
`RIGHT [OUTER] JOIN`, and `ПОЛНОЕ [ВНЕШНЕЕ] СОЕДИНЕНИЕ` / `FULL
[OUTER] JOIN` in source order, each introducing one more source scope. The source list MAY contain several comma-separated
elements, each a source with its own joins; every element after the
first SHALL be rendered as a `CROSS JOIN` in written order, SHALL be
filtered like the base source, and a join condition SHALL see only the
sources of its own element (`UnknownField` otherwise). A comma list
SHALL keep the `*` refusals of joined branches. Joined
branches SHALL support named direct fields and one-hop
reference properties. Each join condition SHALL contain at least one top-level
scalar equality between a direct field or one-hop reference property of the
joined source and one of an earlier source, and MAY combine that anchor with
additional supported scalar predicates over direct fields and one-hop
reference properties of the joined source and earlier sources by top-level
`И`/`AND`. Additional predicates SHALL remain in ON. A `ПОЛНОЕ [ВНЕШНЕЕ]
СОЕДИНЕНИЕ` condition MAY dereference, and the reference join it needs
SHALL be resolved inside the side that owns it. Final `УПОРЯДОЧИТЬ
ПО`/`ORDER BY` SHALL support `ВОЗР`/`ASC` and `УБЫВ`/`DESC` and SHALL
accept a projection alias of the branch as a key, ordering by the aliased
expression. Statements
of a batch SHALL be separated by semicolons and one or more trailing
semicolons SHALL terminate the batch; a single statement remains a valid
batch. Unsupported syntax SHALL fail before execution.

#### Scenario: Logical catalog query
- **WHEN** a query selects `Код` and a custom attribute from
  `Справочник.<name>`
- **THEN** generated SQL uses the DBNames-resolved table and Config-resolved
  physical columns without inferring a numeric name

#### Scenario: Reference property projection
- **WHEN** a query selects `Организация.Код` from a source with a fixed
  `Организация` reference
- **THEN** generated SQL left-joins the SchemaStorage-declared target through
  its ID and projects the target Code column

#### Scenario: Reused reference join
- **WHEN** the same reference path is used in projection, filtering, or ordering
- **THEN** the generated SQL contains one shared join for that source reference

#### Scenario: Implicit source alias and reference property
- **WHEN** a query selects `t.Регистратор.Номер` from a source followed
  directly by alias `t`
- **THEN** the alias qualifies the source and generated SQL left-joins the
  SchemaStorage-declared recorder target to project its Number column

#### Scenario: Explicit source alias
- **WHEN** a source alias follows `КАК` or `AS`
- **THEN** it has the same qualification and SQL-generation semantics as an
  implicit alias

#### Scenario: Clause after an unaliased source
- **WHEN** `ГДЕ`/`WHERE` or `УПОРЯДОЧИТЬ`/`ORDER` immediately follows the source
- **THEN** the clause keyword is not consumed as an implicit alias

#### Scenario: UNION duplicate elimination
- **WHEN** two compatible branches are connected with `ОБЪЕДИНИТЬ` or `UNION`
- **THEN** each branch is compiled independently and PostgreSQL removes
  duplicate combined rows

#### Scenario: UNION ALL duplicate preservation
- **WHEN** compatible branches are connected with `ОБЪЕДИНИТЬ ВСЕ` or
  `UNION ALL`
- **THEN** generated SQL retains duplicate rows

#### Scenario: Union result shape and ordering
- **WHEN** compatible branches have equal logical and expanded SQL projection
  widths followed by final ordering
- **THEN** result labels come from the first branch and ordering addresses the
  combined output rather than a branch table alias

#### Scenario: Incompatible union branches
- **WHEN** a later branch has a different logical or expanded SQL projection
  width
- **THEN** compilation returns a positional diagnostic and no SQL is produced

#### Scenario: INNER JOIN
- **WHEN** two metadata sources use `СОЕДИНЕНИЕ`/`JOIN` or its explicit
  `ВНУТРЕННЕЕ`/`INNER` form with a supported condition
- **THEN** generated PostgreSQL uses INNER JOIN and returns matching pairs only

#### Scenario: LEFT JOIN with a reference projection
- **WHEN** a query selects `Регистратор.Номер` and a right-source field
  through a supported `LEFT JOIN`
- **THEN** generated PostgreSQL preserves every left row, uses the main LEFT
  JOIN, and independently resolves the recorder reference property

#### Scenario: RIGHT JOIN
- **WHEN** two metadata sources use `ПРАВОЕ [ВНЕШНЕЕ] СОЕДИНЕНИЕ` or
  `RIGHT [OUTER] JOIN`
- **THEN** generated PostgreSQL preserves every right row with NULL values for
  an absent left side

#### Scenario: Chained joins
- **WHEN** a branch joins a document, its tabular section, and a catalog with
  three sources in a row, the last condition referencing the first source
- **THEN** generated SQL emits the joins in source order with their own ON
  conditions, followed by any reference-property joins, and every source is
  addressable by its alias

#### Scenario: Same object under two aliases
- **WHEN** a branch joins `Справочник.Контрагенты` twice under different
  aliases
- **THEN** each alias resolves to its own scope and generated SQL contains two
  joins on that table

#### Scenario: FULL JOIN matched and unmatched rows
- **WHEN** two aliased metadata sources are connected by a supported FULL JOIN
- **THEN** the result contains all matching combinations and every unmatched
  row from both sources with NULL values for the absent side

#### Scenario: FULL JOIN transposition
- **WHEN** a FULL JOIN is compiled
- **THEN** no transposition is generated: the SQL carries a native `FULL
  JOIN` between the two sources, and no UNION ALL with an anti-match
  predicate

#### Scenario: FULL JOIN in a chain
- **WHEN** a branch chains a LEFT JOIN and a FULL JOIN in either order, or
  two FULL JOINs
- **THEN** every join is rendered in source order and the result matches
  the platform row for row

#### Scenario: FULL JOIN under aggregation
- **WHEN** a branch aggregates or groups over a FULL JOIN
- **THEN** the aggregate sees the null-extended rows of both sides, as the
  platform answers

#### Scenario: FULL JOIN condition that dereferences
- **WHEN** a FULL JOIN condition compares `Т.Клиент.Наименование` with a
  field of the joined source
- **THEN** the reference join is placed inside the side that owns it, so it
  cannot land after the full join

#### Scenario: FULL JOIN condition that cannot be planned
- **WHEN** a FULL JOIN condition is not an equality chain
- **THEN** compilation fails, because the server plans a full join only on
  merge- or hash-joinable conditions

#### Scenario: FULL JOIN result operators
- **WHEN** a FULL JOIN uses WHERE, DISTINCT, TOP, or final ordering
- **THEN** filtering preserves null-extended row semantics and result-level
  operations apply to the whole joined result

#### Scenario: Status predicates next to the join key
- **WHEN** ON contains a cross-source equality followed by an IN-list of
  catalog `ЗНАЧЕНИЕ` expressions and an enumeration `ЗНАЧЕНИЕ` comparison
- **THEN** generated SQL retains all three predicates in ON in source order

#### Scenario: Outer join one-sided predicate
- **WHEN** an additional predicate refers only to one source of LEFT, RIGHT, or
  FULL JOIN
- **THEN** it remains in ON and is not moved to WHERE

#### Scenario: FULL JOIN predicate transposition
- **WHEN** FULL JOIN contains additional supported predicates
- **THEN** transposed SQL uses a top-level cross-source equality as its
  anti-match marker and applies the complete ON condition in both branches

#### Scenario: Unsupported join shape
- **WHEN** a query combines FULL JOIN with another join in the same branch,
  references a later source from an earlier join condition, uses wildcard
  joined projection, non-scalar join fields, reference paths deeper than
  one hop in ON, a reference property in a FULL JOIN condition, lacks a
  top-level equality with an earlier source, or nests its only equality
  under OR
- **THEN** compilation returns a positional diagnostic and no SQL is produced

#### Scenario: Repeated query terminator
- **WHEN** a valid query ends in more than one semicolon
- **THEN** all trailing semicolons are consumed as terminators and no
  empty statement is reported

#### Scenario: Bounded syntax failure
- **WHEN** a query contains a mutation, unsupported clause, ambiguous
  reference target, path deeper than one hop, or branch-local ordering
  before another union
- **THEN** compilation returns a positional diagnostic and no SQL is produced

#### Scenario: Comma-separated sources
- **WHEN** a branch reads `ИЗ Справочник.А КАК А, Справочник.Б КАК Б
  ГДЕ А.Поле = Б.Поле`
- **THEN** generated SQL is `FROM … AS "А" CROSS JOIN … AS "Б" WHERE …`
  on both providers

#### Scenario: Join inside a comma element
- **WHEN** a branch reads `ИЗ А, Б ЛЕВОЕ СОЕДИНЕНИЕ В ПО В.x = Б.y`
- **THEN** the `LEFT JOIN` follows the `CROSS JOIN` in the written order,
  and a condition naming a field of `А` is an `UnknownField` diagnostic

#### Scenario: Ordering by a projection alias
- **WHEN** `ВЫБРАТЬ ГОД(Дата) КАК Год ИЗ … УПОРЯДОЧИТЬ ПО Год УБЫВ` is
  compiled without joins or grouping
- **THEN** generated SQL orders by the year expression

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
The `open-sdbl-cli` package SHALL provide `open-sdbl console postgres` using
the existing connection and authentication options, with `repl` retained as a
compatibility alias. It SHALL load metadata at startup, accept
semicolon-terminated multiline UTF-8 queries, provide current-session command
history, syntax highlighting, and completion on interactive terminals, protect
interactive Linux terminal editing with `IUTF8`, recover from a byte-invalid
input line, and execute every generated statement in a verified read-only
`READ COMMITTED` transaction.

#### Scenario: Interactive command hint
- **WHEN** the console is active on a capable interactive terminal
- **THEN** a compact footer for `\dt`, `\di`, `\d`, `\refresh`, `\help`, and
  `\q` remains on the final terminal row while normal output scrolls above it

#### Scenario: Syntax highlighting
- **WHEN** the user edits a lexically complete SDBL query
- **THEN** keywords, literals, comments, parameters, and known metadata names
  are displayed with distinct ANSI styles without changing the query text

#### Scenario: Metadata-aware completion
- **WHEN** the user presses Tab after a partial command, keyword, object, or
  field name
- **THEN** the line editor offers case-insensitive candidates derived from
  console commands and the current resolved metadata snapshot

#### Scenario: Recall query history
- **WHEN** the user presses Up or Down in an interactive console
- **THEN** the line editor navigates queries and commands entered earlier in
  the current console session

#### Scenario: Query execution
- **WHEN** the user enters a supported 1C query terminated by `;`
- **THEN** the CLI displays SDBL-to-SQL generation time, exact generated SQL,
  PostgreSQL execution time, column labels, rows, and row count and then
  prompts for the next command

#### Scenario: Recoverable error
- **WHEN** compilation or PostgreSQL execution fails
- **THEN** the console prints the error and elapsed phase time, rolls back the
  statement transaction, and remains available

#### Scenario: Recoverable byte-invalid input
- **WHEN** one input line is not valid UTF-8
- **THEN** the console discards the affected statement, reports the input
  error, and accepts the next command without closing the database connection

#### Scenario: UTF-8 terminal editing
- **WHEN** the console runs on an interactive Linux terminal with `IUTF8`
  disabled
- **THEN** it enables `IUTF8` while reading commands and restores the previous
  terminal attributes before exit

#### Scenario: Session lifecycle
- **WHEN** input reaches EOF or the user enters `\q`
- **THEN** the CLI restores terminal state, closes the PostgreSQL connection,
  and exits successfully

### Requirement: Provide metadata discovery commands
The console SHALL implement `\dt`, `\di`, `\d <metadata-name>`, `\refresh`,
`\help`, and `\q` using the resolved metadata snapshot.

#### Scenario: List tables
- **WHEN** the user enters `\dt`
- **THEN** the console lists logical kind/name, GUID, canonical physical table,
  SchemaStorage status, and live-catalog status

#### Scenario: List indexes
- **WHEN** the user enters `\di`
- **THEN** the console lists owning logical metadata, declared index, live
  index, normalized logical key, and match status

#### Scenario: Describe metadata
- **WHEN** the user enters `\d <qualified-or-unique-name>`
- **THEN** the console displays the object identity followed by its logical
  attributes, physical members/types, and declared/live indexes

### Requirement: Compile application-defined value presentations
The core compiler SHALL support the reference
`.Представление`/`.Presentation` property, the bilingual
`ПРЕДСТАВЛЕНИЕССЫЛКИ`/`REFPRESENTATION` function, and the bilingual
`ПРЕДСТАВЛЕНИЕ`/`PRESENTATION` function in projections. Before SQL generation,
the core SHALL return one deduplicated batch containing every possible
reference target object ID needed by the query. The application SHALL answer
with fields and a structured presentation template for each target. The core
SHALL validate those plans and compile them to PostgreSQL without accepting raw
SQL or metadata names from the application. In a joined branch, a presentation
MAY consume a supported one-hop dereferenced field; its presentation join SHALL
use the dereference alias as its source and SHALL reuse compatible ancestor
joins. Deferred payloads and batch lookup keys SHALL be raw reference bytes
rather than hex text.

#### Scenario: Source reference presentation
- **WHEN** a query applies `ПРЕДСТАВЛЕНИЕССЫЛКИ` to the source `Ссылка`
- **THEN** the request contains the source object GUID and the generated SQL
  applies its returned plan to the source row without an unnecessary join

#### Scenario: Fixed reference field presentation
- **WHEN** a reference field has one SchemaStorage target
- **THEN** the request contains that target GUID and generated SQL uses one
  reusable LEFT JOIN to evaluate the target's returned template

#### Scenario: Multiple possible reference targets
- **WHEN** a pure reference field can contain more than one target type
- **THEN** the request contains every target GUID and generated SQL selects the
  corresponding template by the physical RTRef type discriminator

#### Scenario: Universal reference target
- **WHEN** SchemaStorage declares an empty `R` target and a bounded query
  presents that reference
- **THEN** the main SQL returns the 20-byte `RTRef ‖ RRRef` binary payload as
  a runtime-typed reference column for the projected value and retains its
  predicates and `TOP`/`LIMIT`

#### Scenario: Bounded deferred lookup
- **WHEN** the application resolves deferred payloads from returned rows
- **THEN** it groups them by runtime RTRef object type and uses core-generated
  batch lookup SQL with the validated presentation plan for that object only,
  and the lookup returns the raw 16-byte reference as its key column

#### Scenario: Unknown runtime reference type
- **WHEN** a deferred payload contains an RTRef discriminator absent from the
  metadata snapshot
- **THEN** resolution fails explicitly instead of choosing a table by name or
  rendering the raw binary reference as a presentation

#### Scenario: Scalar REFPRESENTATION
- **WHEN** `ПРЕДСТАВЛЕНИЕССЫЛКИ` receives a non-reference expression
- **THEN** it preserves that expression's value and does not request a plan

#### Scenario: Scalar PRESENTATION
- **WHEN** `ПРЕДСТАВЛЕНИЕ` receives a non-reference expression such as `4`
- **THEN** generated SQL converts it to text and the logical result is `"4"`

#### Scenario: Identifier-only callback protocol
- **WHEN** the application receives a presentation request or returns a plan
- **THEN** objects and custom attributes are identified by real metadata GUIDs
  and standard fields by stable numeric IDs, with no table or field name in the
  protocol

#### Scenario: Safe template
- **WHEN** the application returns a concatenation of field IDs and literal
  text
- **THEN** the core quotes SQL identifiers and literals itself and emits no
  application-provided raw SQL

#### Scenario: Invalid presentation plan
- **WHEN** a plan is missing, references a field outside its target object, or
  has an invalid expression shape
- **THEN** compilation fails with a typed diagnostic before database execution

#### Scenario: Presentation of a dereferenced JOIN field
- **WHEN** a joined projection presents `Ссылка.ДоговорКонтрагента` or
  `ЦФО.Сам_БизнесРегион`
- **THEN** generated SQL first joins the owner of the selected property and
  then joins the property's presentation target from that owner alias

#### Scenario: Reused dereference ancestor
- **WHEN** ordinary projection and presentation require the same first-hop
  dereference
- **THEN** generated SQL contains one shared ancestor join followed by only the
  required presentation joins

#### Scenario: Presentation through joins and unions
- **WHEN** presentation projections occur in supported JOIN, transposed FULL
  JOIN, or compatible UNION branches
- **THEN** each branch retains its validated bindings and compatible output
  shape

### Requirement: Cache CLI presentation plans
The console application SHALL cache presentation plans in a bounded async Moka
cache keyed by metadata generation, object ID, language, and policy version.
Metadata refresh SHALL change the generation and prevent reuse of stale plans.
The cache SHALL remain outside the core crate.

#### Scenario: Repeated presentation query
- **WHEN** two console queries in one metadata generation request the same
  object, language, and policy
- **THEN** the CLI provider reuses the cached plan

#### Scenario: Metadata refresh
- **WHEN** `\\refresh` installs a new metadata snapshot
- **THEN** subsequent presentation planning cannot observe plans cached for the
  prior generation

### Requirement: Provide kind-specific default CLI presentations
The console's default presentation provider SHALL select a structured template
by the requested target object's resolved metadata kind. A catalog with live
Description and Code fields SHALL use `Наименование (Код)`. A document with
live Number and Date fields SHALL use `<Тип> <Номер> от <Период>`, where
`<Тип>` is the Russian Config synonym falling back to the metadata name, and
`<Период>` is the standard document `Дата`/`Date` field. Missing optional
fields SHALL use deterministic non-failing fallbacks.

#### Scenario: Catalog reference
- **WHEN** the CLI resolves a catalog target exposing Description and Code
- **THEN** its structured plan concatenates Description, `" ("`, Code, and
  `")"`

#### Scenario: Document reference
- **WHEN** the CLI resolves a document target exposing Number and Date
- **THEN** its structured plan concatenates localized document type, `" "`,
  Number, `" от "`, and Date

#### Scenario: Internal callback identities
- **WHEN** either default template is returned to the core
- **THEN** every field remains a numeric standard-field ID and only separator
  and type presentation text is represented as a literal

### Requirement: Compile source-free scalar SELECT branches
The `open-sdbl` library SHALL compile one or more
`ВЫБРАТЬ`/`SELECT` branches into one PostgreSQL SELECT statement using only
authoritative resolved metadata. A branch MAY omit `ИЗ`/`FROM` when every
projection is a source-independent bounded scalar expression. Such projections
SHALL support literals, parentheses, unary operators, bounded arithmetic and
logical operators, and literal calls to `ПРЕДСТАВЛЕНИЕ`/`PRESENTATION` or
`ПРЕДСТАВЛЕНИЕССЫЛКИ`/`REFPRESENTATION`. Their output SHALL keep the native
type of the expression and report it as the column kind; only presentation
calls convert their argument to text. A source-free branch SHALL reject
fields, wildcards, joins, and source-dependent clauses before execution.
Source-backed branches SHALL retain all previously specified projection,
source, JOIN, UNION, filtering, ordering, and diagnostic behavior.

#### Scenario: Source-free numeric literal
- **WHEN** the query is `SELECT 4;`
- **THEN** generated SQL selects the numeric value `4` without a FROM clause
  and the column kind is number

#### Scenario: Source-free scalar presentation
- **WHEN** the query is `SELECT ПРЕДСТАВЛЕНИЕ(4);`
- **THEN** generated SQL selects textual value `4` without requesting a
  reference presentation plan

#### Scenario: Source-free field rejection
- **WHEN** a source-free branch projects an identifier
- **THEN** compilation reports that a field requires FROM and produces no SQL

### Requirement: Compile bounded COUNT projections
The compiler SHALL accept bilingual `COUNT`/`КОЛИЧЕСТВО` projections with
`*`, one resolved field, or `DISTINCT`/`РАЗЛИЧНЫЕ` followed by one resolved
field. It SHALL compile a native COUNT whose column kind is number. A pure
compound reference SHALL count its RRef value member. Other compound fields
and COUNT over a transposed FULL JOIN SHALL fail before execution.

#### Scenario: Count all catalog rows
- **WHEN** a query selects `COUNT(*)` from a resolved catalog
- **THEN** generated SQL counts all filtered source rows and the CLI receives
  one numeric aggregate value

#### Scenario: Count distinct field values
- **WHEN** a query selects `КОЛИЧЕСТВО(РАЗЛИЧНЫЕ Код)`
- **THEN** generated PostgreSQL uses `COUNT(DISTINCT <resolved Code column>)`

#### Scenario: Unsafe FULL JOIN count
- **WHEN** a query projects COUNT from a FULL JOIN that is transposed to UNION
  ALL
- **THEN** compilation reports the unsupported aggregate shape and emits no SQL

### Requirement: Compile SUM, MIN, and MAX projections
The compiler SHALL accept bilingual `SUM`/`СУММА`, `MIN`/`МИНИМУМ`, and
`MAX`/`МАКСИМУМ` with one resolved field argument and compile the corresponding
native aggregate. `SUM` SHALL report a number kind and `MIN`/`MAX` SHALL
report the kind of their argument column. `COUNT(DISTINCT field)` and its
Russian form SHALL remain supported. Wildcard and DISTINCT SHALL be accepted
only for COUNT. All aggregates SHALL share the existing compound-field,
projection-mixing, and transposed FULL JOIN safety checks.

#### Scenario: Numeric sum
- **WHEN** a query selects `СУММА(<numeric-field>)`
- **THEN** generated SQL applies SUM to the resolved physical column without a
  text conversion

#### Scenario: Minimum and maximum
- **WHEN** a query selects `MIN(field)` and `МАКСИМУМ(field)`
- **THEN** generated SQL returns both aggregate values in the field's native
  type

#### Scenario: Distinct count remains supported
- **WHEN** a query selects `COUNT(DISTINCT field)`
- **THEN** generated PostgreSQL retains DISTINCT inside COUNT

#### Scenario: Invalid SUM wildcard
- **WHEN** a query uses `SUM(*)`
- **THEN** compilation reports that wildcard is supported only by COUNT

### Requirement: Compile information-register SliceLast sources
The compiler SHALL accept bilingual
`InformationRegister.<name>.SliceLast([period][, condition])` and
`РегистрСведений.<name>.СрезПоследних([period][, condition])` sources.
It SHALL resolve the main table, Period field, Config-declared dimensions, and
data separators through the metadata snapshot. PostgreSQL generation SHALL
select every row at the greatest eligible Period in each dimension/separator
partition. The optional period SHALL be a scalar literal and the optional
condition SHALL use only direct fields and the existing bounded expression
operators. Unsupported or non-information-register use SHALL fail before SQL
execution.

#### Scenario: Current latest slice
- **WHEN** an empty-argument SliceLast source is queried
- **THEN** generated PostgreSQL returns rows at the greatest Period for every
  authoritative dimension and data-separator combination

#### Scenario: Tied latest records
- **WHEN** more than one record in a partition has the greatest eligible Period
- **THEN** every tied record remains in the slice

#### Scenario: Period boundary
- **WHEN** SliceLast receives a scalar period literal
- **THEN** the Period upper bound is applied before greatest-period selection

#### Scenario: Virtual condition precedes slicing
- **WHEN** a condition is passed as the second SliceLast parameter
- **THEN** it filters candidate records before greatest-period selection

#### Scenario: WHERE follows slicing
- **WHEN** an ordinary WHERE follows a SliceLast source
- **THEN** it filters the already selected latest rows and cannot reveal an
  older record

#### Scenario: Joined SliceLast source
- **WHEN** either side of a supported JOIN is an information-register SliceLast
  source
- **THEN** the derived relation participates with the same alias and field
  resolution behavior as its main metadata source

#### Scenario: Invalid SliceLast source
- **WHEN** SliceLast is applied to another metadata kind, a table without
  Period, a parameter period, or a condition containing a reference-property
  dereference
- **THEN** compilation returns a positional diagnostic and no SQL

### Requirement: Compile information-register SliceFirst sources
The compiler SHALL accept bilingual
`InformationRegister.<name>.SliceFirst([period][, condition])` and
`РегистрСведений.<name>.СрезПервых([period][, condition])` sources.
It SHALL resolve the main table, Period, Config-declared dimensions, and data
separators through the metadata snapshot. PostgreSQL generation SHALL retain
every row at the least eligible Period in each dimension/separator partition.
The optional period SHALL be a scalar literal inclusive lower bound, and the
optional condition SHALL use only direct fields and existing bounded expression
operators. Unsupported or non-information-register use SHALL fail before SQL
execution.

#### Scenario: Earliest slice
- **WHEN** an empty-argument SliceFirst source is queried
- **THEN** PostgreSQL ranks Period ascending within every authoritative
  dimension and data-separator partition

#### Scenario: Tied earliest records
- **WHEN** more than one record in a partition has the least eligible Period
- **THEN** every tied record remains in the slice

#### Scenario: Inclusive period boundary
- **WHEN** SliceFirst receives a scalar period literal
- **THEN** candidates are restricted to Period greater than or equal to that
  literal before least-period selection

#### Scenario: Filter placement
- **WHEN** SliceFirst receives a virtual condition and is followed by WHERE
- **THEN** the virtual condition filters candidates before ranking and WHERE
  filters the completed earliest slice

#### Scenario: Joined SliceFirst source
- **WHEN** either side of a supported JOIN is an information-register
  SliceFirst source
- **THEN** the derived relation retains normal alias and field-resolution
  behavior

#### Scenario: SliceLast compatibility
- **WHEN** an existing SliceLast query is compiled after directional
  generalization
- **THEN** it retains descending order and an inclusive upper period bound

#### Scenario: Invalid SliceFirst source
- **WHEN** SliceFirst is applied to another metadata kind, a table without
  Period, a parameter period, or a condition containing a reference-property
  dereference
- **THEN** compilation returns a positional diagnostic and no SQL

### Requirement: Compile accumulation-register Balance sources
The compiler SHALL accept bilingual
`AccumulationRegister.<name>.Balance([period][, condition])` and
`РегистрНакопления.<name>.Остатки([period][, condition])` sources for
balance registers. It SHALL resolve the register's balance-totals table only
from the same object GUID's `DBNames` `AccumRgT` entry and require that table in
SchemaStorage and the live catalog. It SHALL group totals by Config dimensions
and data separators, merge split totals, expose each Config resource with
`Balance`/`Остаток` suffix aliases, and remove groups whose every balance is
zero. An optional scalar period literal SHALL be an exclusive upper boundary;
historical balances SHALL start from a stored totals anchor and apply only the
bounded signed movement delta. An optional direct dimension/separator condition
SHALL be applied before aggregation.

#### Scenario: Current balances
- **WHEN** Balance is called without a period
- **THEN** only the latest `_AccumRgT*` totals period contributes and split
  rows are merged by dimensions

#### Scenario: Balance at a point
- **WHEN** Balance receives a period literal
- **THEN** a stored totals anchor is combined with active movements so only
  movements strictly before that point affect the result

#### Scenario: Zero balance
- **WHEN** every resource sum for one dimension combination is zero
- **THEN** that combination is absent from the Balance result

#### Scenario: Balance filter placement
- **WHEN** Balance receives a virtual condition and is followed by WHERE
- **THEN** the virtual condition restricts both totals and movement branches
  before aggregation and WHERE filters aggregated balances

#### Scenario: Invalid balance register
- **WHEN** Balance is used on a non-accumulation object, a turnover-only
  register, or a register without a matching declared and live `AccumRgT` table
- **THEN** compilation returns a diagnostic and no SQL

### Requirement: Compile accumulation-register Turnovers sources
The compiler SHALL accept bilingual
`AccumulationRegister.<name>.Turnovers([begin][, end][, periodicity][,
condition])` and the corresponding `РегистрНакопления.<name>.Обороты`
source for balance and turnover-only registers. It SHALL group active movement
rows by Config dimensions and data separators and expose Config resources with
`Turnover`/`Оборот` suffix aliases. A balance register SHALL apply movement
direction, while a turnover-only register SHALL sum stored resource values. The
optional scalar begin and end literals SHALL define a half-open interval. The
initial bounded subset SHALL require the periodicity slot to be omitted and
SHALL accept a direct dimension/separator condition in the fourth slot.

#### Scenario: All-time turnovers
- **WHEN** Turnovers is called without arguments
- **THEN** active resource movements are aggregated by dimensions

#### Scenario: Bounded turnovers
- **WHEN** begin and end literals are provided
- **THEN** generated PostgreSQL applies `Period >= begin` and `Period < end`
  before aggregation

#### Scenario: Turnover filter
- **WHEN** the fourth condition parameter is provided with an omitted
  periodicity slot
- **THEN** it restricts direct dimension/separator fields before aggregation

#### Scenario: Joined aggregate source
- **WHEN** Balance or Turnovers participates in a supported JOIN
- **THEN** its derived relation retains ordinary alias, reference-property, and
  outer-filter behavior

#### Scenario: Unsupported periodicity
- **WHEN** the third Turnovers parameter is nonempty
- **THEN** compilation reports unsupported periodic grouping and emits no SQL

### Requirement: Complete qualified virtual-table sources
The interactive console SHALL derive virtual-table completion candidates from
the resolved metadata kind. It SHALL offer Russian and English virtual-table
names after bare, Russian-kind-qualified, and English-kind-qualified register
object spellings, including an empty argument list accepted by the parser. It
SHALL NOT offer register virtual tables for unrelated metadata kinds.

#### Scenario: Accumulation-register virtual completion
- **WHEN** Tab follows a partial qualified accumulation-register source
- **THEN** completion offers `Остатки()`/`Balance()` and
  `Обороты()`/`Turnovers()` candidates for that object

#### Scenario: Information-register virtual completion
- **WHEN** Tab follows a partial qualified information-register source
- **THEN** completion offers `СрезПоследних()`/`SliceLast()` and
  `СрезПервых()`/`SliceFirst()` candidates for that object

#### Scenario: Non-register object completion
- **WHEN** completion candidates are built for a catalog or another unrelated
  metadata kind
- **THEN** no register virtual-table suffix is attached to that object

### Requirement: Route CLI PostgreSQL connections through an optional SOCKS5 proxy
The `open-sdbl-cli` package SHALL accept `--socks5-proxy HOST:PORT` for
`metadata postgres`, `console postgres`, and the `repl` compatibility alias.
When present, the CLI SHALL establish the PostgreSQL byte stream with the
SOCKS5 CONNECT command using the no-authentication method. It SHALL send a
non-IP PostgreSQL host to the proxy as a domain name rather than resolving it
locally. When absent, the CLI SHALL retain its direct PostgreSQL connection
behavior.

#### Scenario: Proxied metadata connection
- **WHEN** `metadata postgres` receives a valid SOCKS5 proxy endpoint
- **THEN** its metadata session uses a SOCKS5 CONNECT tunnel to the requested
  PostgreSQL host and port

#### Scenario: Proxied console connection
- **WHEN** `console postgres` or `repl postgres` receives a valid SOCKS5 proxy
  endpoint
- **THEN** its complete PostgreSQL session uses the negotiated SOCKS5 tunnel

#### Scenario: Proxy-side database name resolution
- **WHEN** the PostgreSQL host is a DNS name and a SOCKS5 proxy is configured
- **THEN** the CONNECT request carries that name in SOCKS5 domain-address form

#### Scenario: Direct connection compatibility
- **WHEN** no SOCKS5 proxy option is provided
- **THEN** the CLI connects directly with the existing PostgreSQL connection
  and authentication options

#### Scenario: Invalid proxy endpoint
- **WHEN** the proxy value lacks a host, lacks a valid nonzero port, or contains
  an unbracketed IPv6 address
- **THEN** the CLI reports a usage error before attempting a connection

#### Scenario: Proxy negotiation failure
- **WHEN** the proxy cannot be reached, requires another authentication method,
  times out, or rejects the CONNECT request
- **THEN** the CLI reports a SOCKS5 connection error without exposing database
  credentials

### Requirement: Bound proxied PostgreSQL startup
After a SOCKS5 proxy accepts the CONNECT request, the `open-sdbl-cli` package
SHALL apply its connection timeout to PostgreSQL startup and authentication over
the tunneled stream. Expiration SHALL close the incomplete stream and report
that PostgreSQL startup through SOCKS5 timed out. This deadline SHALL NOT alter
the existing direct connection path.

#### Scenario: Silent database target after SOCKS5 CONNECT
- **WHEN** the proxy establishes the requested tunnel but the database endpoint
  returns no PostgreSQL startup or authentication response
- **THEN** the CLI terminates the connection attempt after the configured
  connection timeout with a PostgreSQL-through-SOCKS5 timeout error

#### Scenario: Responsive PostgreSQL startup
- **WHEN** PostgreSQL completes startup and authentication within the deadline
- **THEN** the CLI starts the connection driver and continues metadata loading

#### Scenario: Direct connection compatibility
- **WHEN** no SOCKS5 proxy is configured
- **THEN** the existing `tokio-postgres` direct connection behavior remains in
  effect

### Requirement: Build the completion catalog from indexed metadata
The interactive console SHALL project queryable fields at most once per live
named object during each completion-catalog rebuild, resolve reference targets
through indexed physical-table identity, and deduplicate candidates through
normalized membership rather than repeated full candidate scans. It SHALL
preserve the existing commands, keywords, object spellings, field aliases,
qualified fields, reference paths, case-insensitive uniqueness, and sorted
completion behavior.

#### Scenario: Duplicate spelling with different case
- **WHEN** multiple metadata sources contribute candidates differing only by
  case
- **THEN** only the first spelling is retained, as before

#### Scenario: Qualified field completion
- **WHEN** a resolved object exposes a queryable field alias
- **THEN** completion includes the same bare and object-qualified candidates

#### Scenario: Reference path completion
- **WHEN** a queryable reference field has a uniquely resolved target object
- **THEN** completion includes the same source-alias and target-alias paths
  without reprojecting the target for each reference field

### Requirement: Report metadata-loading progress without contaminating output
During PostgreSQL metadata acquisition, the CLI SHALL show a rate-limited
progress bar with phase, completed/total Config resources, compressed bytes,
percentage, and elapsed completion summary only when standard error is a
terminal. Progress SHALL be written to standard error. Standard output and
non-TTY standard error SHALL contain no progress rendering or terminal control
sequences.

#### Scenario: Interactive metadata loading
- **WHEN** the CLI acquires metadata with standard error attached to a terminal
- **THEN** the user sees phase changes and Config completion advance to 100%

#### Scenario: Redirected metadata snapshot
- **WHEN** `open-sdbl metadata postgres` stdout is redirected or piped
- **THEN** stdout contains only the existing tabular snapshot records

#### Scenario: Noninteractive diagnostics
- **WHEN** standard error is not a terminal
- **THEN** progress rendering is suppressed while ordinary errors remain
  available on standard error

### Requirement: Complete query sources through the metadata hierarchy
The interactive console SHALL use a dedicated source-completion catalog after
`ИЗ`/`FROM` and `СОЕДИНЕНИЕ`/`JOIN`. Every ordinary source candidate SHALL
have the form `Type.MetadataName`, where `Type` is the resolved object's
Russian or English metadata kind. Source completion SHALL exclude bare object
names, field aliases, reference paths, and physical PostgreSQL table names.

#### Scenario: Empty source prefix
- **WHEN** the user presses Tab immediately after `ИЗ` or `FROM`
- **THEN** every metadata candidate is qualified as `Type.MetadataName`

#### Scenario: Partial metadata type
- **WHEN** the user enters a partial type after a source keyword
- **THEN** completion offers matching qualified objects of that type and does
  not offer fields or physical tables

#### Scenario: Register virtual source
- **WHEN** a qualified register source is followed by a partial virtual-table
  suffix
- **THEN** completion offers only virtual tables valid for that register kind

#### Scenario: Non-source field completion
- **WHEN** completion occurs outside a source position
- **THEN** field aliases and qualified reference paths remain available

### Requirement: Escape untrusted data in terminal output
Every CLI output path that prints database-derived or metadata-derived
text SHALL escape control characters and Unicode bidirectional override
characters so that stored data cannot inject terminal escape sequences,
and SHALL bound the number of printed rows and the width of each cell.

#### Scenario: Stored escape sequence
- **WHEN** a query result cell contains an ESC-initiated control
  sequence or an OSC payload
- **THEN** the printed cell shows an escaped textual form and the
  terminal state (screen, title, clipboard) is unaffected

#### Scenario: Oversized result set
- **WHEN** a query returns more rows than the display limit
- **THEN** the CLI prints up to the limit plus a trailer stating how
  many rows were omitted

#### Scenario: Narrow terminal
- **WHEN** the terminal is too narrow to display every result column
- **THEN** the CLI prints the fitting prefix and a trailer stating how many
  columns were omitted

### Requirement: Secure PostgreSQL transport by default
The CLI SHALL support TLS for PostgreSQL connections with certificate
and hostname verification as the default mode, SHALL honor `PGSSLMODE`
when no flag overrides it, and SHALL refuse plaintext connections unless
the user explicitly opts in.

#### Scenario: Default connection
- **WHEN** the user connects without transport flags
- **THEN** the connection uses TLS with full verification or fails with
  a diagnostic; it never silently falls back to plaintext

#### Scenario: Explicit plaintext opt-in
- **WHEN** the user passes the plaintext mode without the explicit
  insecure opt-in flag
- **THEN** the CLI refuses to connect and names the required flag

#### Scenario: Private PostgreSQL CA
- **WHEN** a private CA file is supplied in `verify-ca` or `verify-full` mode
- **THEN** PostgreSQL TLS validates the server chain against that CA instead
  of disabling certificate verification

### Requirement: Warn when certificate verification is disabled
Disabling MSSQL certificate verification SHALL emit a visible warning on
every use, and the CLI SHALL offer trusting a specific CA file as the
safe alternative for self-signed deployments.

#### Scenario: Trust flag warning
- **WHEN** the user passes the trust-server-certificate flag
- **THEN** a warning naming the risk is written to standard error before
  connecting

### Requirement: Keep database sessions read-only
Every database session SHALL establish provider-enforced read-only
semantics where the provider offers them, and SHALL recover from a failed
query without leaving an open transaction behind: a failed rollback SHALL
poison the session instead of letting it be reused.

The session SHALL NOT probe the server for its own state before a read —
neither role membership, nor isolation level, nor transaction count. Those
checks guard against writes the CLI cannot make: the compiler generates
`SELECT` statements only, and provisioning the account is the operator's
decision.

#### Scenario: PostgreSQL session
- **WHEN** a query is executed over a PostgreSQL session
- **THEN** it runs inside a `READ COMMITTED READ ONLY` transaction

#### Scenario: MSSQL verification
- **WHEN** a query is executed over an MSSQL session
- **THEN** no verification statement is sent first: the session opens its
  own transaction, reads, and rolls back

#### Scenario: Failed rollback
- **WHEN** a rollback after a failed query itself fails
- **THEN** the session is not reused; the CLI reports the state and
  reconnects or exits

#### Scenario: Login with wider rights
- **WHEN** the login is a member of `db_owner`, or the session runs at an
  isolation level other than read committed
- **THEN** the query runs, because neither changes what the CLI sends

### Requirement: Cancel and bound in-flight queries
Query execution SHALL be cancellable from the keyboard without killing
the process, SHALL restore the terminal state on interruption, and SHALL
be bounded by client-side and server-side timeouts.

#### Scenario: Interrupted query
- **WHEN** the user presses Ctrl-C while a query is executing
- **THEN** the in-flight query is cancelled on the server, the terminal
  is restored, and the REPL returns to its prompt

#### Scenario: Stalled server
- **WHEN** the server stops responding after the handshake
- **THEN** the operation fails with a timeout diagnostic instead of
  hanging indefinitely

### Requirement: Handle credentials without lingering copies
The CLI SHALL read password files through a single opened descriptor
(verifying file type, ownership, and permissions on that descriptor),
SHALL zeroize password material it owns after use, SHALL remove
credential environment variables from the process environment once
consumed, and SHALL NOT accept passwords through command-line
arguments.

#### Scenario: Password file indirection
- **WHEN** the password file path is replaced between check and use
- **THEN** the CLI's checks and its read operate on the same opened
  file, so the substitution cannot bypass validation

#### Scenario: No password flag
- **WHEN** the user passes any password-bearing command-line flag
- **THEN** the CLI rejects it and points at the supported credential
  channels

### Requirement: Authenticate to SOCKS5 proxies
The SOCKS5 client SHALL support username/password authentication in
addition to unauthenticated access, and SHALL surface the proxy's reply
code when a connection is refused.

#### Scenario: Authenticated proxy
- **WHEN** the proxy offers only username/password authentication and
  credentials are configured
- **THEN** the tunnel is established using RFC 1929 authentication

#### Scenario: Authentication downgrade
- **WHEN** proxy credentials are configured
- **THEN** the client does not advertise anonymous authentication and rejects
  a proxy that selects it

### Requirement: Discover extension and service sources
Metadata discovery and completion SHALL surface change-registration,
calculation-kind dependency, and extra-dimension sources under their
owning objects, and SHALL list extension-added attributes alongside base
attributes of the extended object.

#### Scenario: Completing a change-registration source
- **WHEN** the user completes a FROM clause for a registered object
- **THEN** the change-registration source spelling is offered

#### Scenario: Describing an extended object
- **WHEN** the user describes an object extended by a configuration
  extension
- **THEN** the output lists extension-added attributes with their
  extension origin

### Requirement: Render typed cells client-side
The CLI SHALL read query results as typed cells on both providers instead of
requesting textual conversion from the database, SHALL resolve deferred
presentations from raw reference bytes, and SHALL render cells with one
provider-independent policy: binary values as `0x` followed by upper-case
hexadecimal digits, booleans as `true`/`false`, date-times as
`YYYY-MM-DD HH:MM:SS` without fractional seconds, numbers with their declared
scale, UUIDs in canonical lower-case form, and `NULL` for absent values. The
CLI SHALL NOT add production dependencies for this decoding.

#### Scenario: PostgreSQL reference cell
- **WHEN** a PostgreSQL query returns a `bytea` reference column
- **THEN** the CLI prints the bytes as `0x…` in upper-case hexadecimal

#### Scenario: MSSQL numeric cell
- **WHEN** an MSSQL query returns a `numeric(15,2)` value `15.5`
- **THEN** the CLI prints `15.50`

#### Scenario: Timestamp cell
- **WHEN** either provider returns a date-time value
- **THEN** the CLI prints it as `YYYY-MM-DD HH:MM:SS`

#### Scenario: Unsupported PostgreSQL wire type
- **WHEN** a PostgreSQL result contains a column type the CLI cannot decode
- **THEN** the CLI reports a data error naming the type instead of printing
  garbage

### Requirement: Compile reference UUID expressions
The compiler SHALL accept bilingual `УНИКАЛЬНЫЙИДЕНТИФИКАТОР`/`UUID` with
exactly one field argument that resolves to a reference member and SHALL
compile it with pure SQL into PostgreSQL `uuid` or MSSQL `uniqueidentifier`
in canonical 1C field order, reporting the column kind as UUID. `NULL`
references SHALL yield `NULL`. The expression SHALL be usable wherever scalar
expressions are, including projections and predicates.

#### Scenario: Source reference
- **WHEN** a query projects `УНИКАЛЬНЫЙИДЕНТИФИКАТОР(Ссылка)` from a catalog
- **THEN** PostgreSQL SQL reorders the `_IDRRef` bytes with `substring` and
  casts the hex text to `uuid`, and MSSQL SQL reverses the first three groups
  and casts to `uniqueidentifier`

#### Scenario: Known GUID
- **WHEN** the physical reference bytes are `9022249e3a1ac4b94be8faddd2f8bde9`
- **THEN** both databases return `d2f8bde9-fadd-4be8-9022-249e3a1ac4b9`

#### Scenario: Dereferenced and compound arguments
- **WHEN** the argument is `Ссылка.Владелец` or a compound field with an
  `RRRef` member
- **THEN** the dereference join is reused and only the `RRRef` member is
  decoded

#### Scenario: Invalid argument
- **WHEN** the argument is a non-reference field, a literal, a `ЗНАЧЕНИЕ`
  expression, or the query has no FROM
- **THEN** compilation returns a positional diagnostic and emits no SQL

### Requirement: Compile bounded SDBL to Microsoft SQL Server
The core library SHALL expose MSSQL compile and prepare APIs that reuse the
existing bounded SDBL parser, metadata resolution, and presentation-plan
protocol while emitting native T-SQL. Existing PostgreSQL APIs and generated
PostgreSQL SQL SHALL remain compatible.

#### Scenario: MSSQL projection and limit
- **WHEN** an MSSQL query selects logical 1C fields with `ПЕРВЫЕ`/`TOP`
- **THEN** generated T-SQL quotes physical identifiers, converts supported
  text-rendered values, and applies `TOP` in SQL Server

#### Scenario: MSSQL rowversion projection
- **WHEN** a selected logical field is backed by an MSSQL `timestamp` or
  `rowversion` column
- **THEN** generated T-SQL projects the physical column without `CAST` or
  `CONVERT`, and the CLI renders the received binary value as hexadecimal text

#### Scenario: Binary literal comparison
- **WHEN** a filter compares a binary or rowversion field with a validated
  `0x` hexadecimal literal
- **THEN** MSSQL T-SQL contains a native varbinary literal and PostgreSQL SQL
  contains an equivalent `bytea` literal without treating the value as text

#### Scenario: Presentation from a configuration-extension table
- **WHEN** an MSSQL reference target has a canonical physical table and one or
  more `X`-suffixed configuration-extension table variants
- **THEN** direct reads, dereferences, and presentation joins use one
  deterministic `UNION ALL` relation over the canonical and exact extension
  variants, allowing referenced rows redirected by 1C to resolve normally

#### Scenario: Unicode and binary values
- **WHEN** an MSSQL query contains Cyrillic strings or compares reference type
  discriminators
- **THEN** generated T-SQL uses Unicode string literals and native varbinary
  literals without PostgreSQL casts

#### Scenario: Dialect isolation
- **WHEN** the same supported SDBL is compiled for PostgreSQL and MSSQL
- **THEN** each output uses its native limit, cast, literal, aggregate, and
  virtual-table syntax without textual post-processing

### Requirement: Provide a read-only MSSQL console
The CLI SHALL provide `open-sdbl console mssql` and `open-sdbl metadata mssql`
using SQL Server authentication, TDS over direct TCP or the existing SOCKS5
transport, TLS certificate validation by default, and an optional explicit
server-certificate trust flag. The password SHALL be read from
`MSSQL_PASSWORD`. The console SHALL request read-only application intent and
execute only fixed metadata SELECTs or SQL generated from the bounded SELECT
compiler.

#### Scenario: MSSQL connection defaults
- **WHEN** a user supplies host, database, user, and `MSSQL_PASSWORD`
- **THEN** the CLI connects to port 1433 with TLS validation and read-only
  application intent

#### Scenario: Explicit certificate trust
- **WHEN** the server uses an untrusted certificate and the user supplies
  `--trust-server-certificate`
- **THEN** the CLI opts out of certificate validation for that connection and
  documents the security tradeoff

#### Scenario: Missing password
- **WHEN** SQL authentication is requested without `MSSQL_PASSWORD`
- **THEN** the CLI fails before opening a connection and does not print a
  password value

#### Scenario: Query execution
- **WHEN** the user enters a supported semicolon-terminated SDBL SELECT
- **THEN** the console prints native T-SQL, execution time, textual columns,
  rows, and row count and remains available after recoverable errors

#### Scenario: Defense in depth
- **WHEN** the MSSQL provider is deployed
- **THEN** documentation requires a SQL login whose effective permissions are
  limited to SELECT because read-only application intent is not authorization

### Requirement: Preserve trusted CLI diagnostic layout
Top-level argument and usage diagnostics SHALL render built-in help layout with
real line breaks, while errors containing database-, metadata-, parser-, or
operating-system-derived text SHALL remain escaped before reaching the
terminal. The missing PostgreSQL plaintext opt-in diagnostic SHALL use stable
plain-text fields consisting of an error code, summary, cause, and remediation,
without ANSI styling or the complete command manual.

#### Scenario: Missing plaintext opt-in
- **WHEN** PostgreSQL plaintext mode is requested without the explicit
  insecure opt-in flag
- **THEN** the diagnostic identifies a stable machine-readable code and gives a
  human-readable explanation plus exact alternatives to add the opt-in or
  restore verified TLS

#### Scenario: External error text
- **WHEN** a non-usage error contains terminal control characters
- **THEN** those characters are rendered in escaped textual form

### Requirement: Construct date values
The compiler SHALL accept `ДАТАВРЕМЯ`/`DATETIME` with integer year, month, and
day components followed by optional hour, minute, and second components. It
SHALL validate the calendar value and generate a typed date expression for
PostgreSQL and MSSQL. MSSQL generation SHALL apply the configured `_YearOffset`
when the expression participates in a source-backed query and SHALL remove that
offset from projected logical output.

#### Scenario: Date-only constructor
- **WHEN** `ДАТАВРЕМЯ` receives year, month, and day
- **THEN** the omitted time is midnight and generated SQL contains a typed date
  rather than an untyped user-concatenated literal

#### Scenario: Date-time constructor
- **WHEN** `DATETIME` receives all six valid integer components
- **THEN** generated SQL preserves the exact hour, minute, and second

#### Scenario: Invalid constructor
- **WHEN** component count, numeric form, range, calendar date, or offset MSSQL
  year is invalid
- **THEN** compilation returns a positional diagnostic and emits no SQL

### Requirement: Calculate beginning-of-period values
The compiler SHALL accept `НАЧАЛОПЕРИОДА`/`BEGINOFPERIOD` with a date expression
and one of the bilingual minute, hour, day, week, ten-day, month, quarter,
half-year, or year period identifiers. It SHALL generate equivalent native SQL
for PostgreSQL and MSSQL in projections and predicates. Week SHALL begin on
Monday until regional first-weekday metadata becomes part of the compiler
input. A known period the function does not accept (`СЕКУНДА`) SHALL be a
`Syntax` diagnostic; an unknown period name SHALL be an `UnsupportedFeature`
diagnostic. Virtual-table period arguments SHALL accept `ДАТАВРЕМЯ`, a date
parameter, and `НАЧАЛОПЕРИОДА`, `КОНЕЦПЕРИОДА`, or `ДОБАВИТЬКДАТЕ` nested
over those (the count of `ДОБАВИТЬКДАТЕ` being a numeric literal or
parameter), compiled in the physical storage date domain.

#### Scenario: Nested date constructor
- **WHEN** `НАЧАЛОПЕРИОДА` wraps a `ДАТАВРЕМЯ` expression
- **THEN** the nested typed date is truncated to the requested boundary

#### Scenario: Source field boundary
- **WHEN** a source-backed projection or filter applies `НАЧАЛОПЕРИОДА` to a
  date field
- **THEN** generated SQL evaluates the function in the database and preserves
  MSSQL year-offset semantics

#### Scenario: Virtual-table date argument
- **WHEN** `Обороты(НАЧАЛОПЕРИОДА(&П, МЕСЯЦ), КОНЕЦПЕРИОДА(&П, МЕСЯЦ))`
  is compiled with a date value for `&П`
- **THEN** both bounds render the inlined date in the storage domain,
  truncated to the month and extended to its last second

#### Scenario: Unknown period
- **WHEN** the second argument is absent or is not a supported period identifier
- **THEN** compilation returns a positional diagnostic and emits no SQL

#### Scenario: Second period
- **WHEN** `НАЧАЛОПЕРИОДА(Дата, СЕКУНДА)` is compiled
- **THEN** compilation fails with a `Syntax` diagnostic at the period token

### Requirement: Compile IN-list predicates

The compiler SHALL accept bilingual `В`/`IN` after a scalar expression and a
non-empty parenthesized, comma-separated list of scalar expressions. It SHALL
preserve list order, compile each member using the left operand's type context,
and emit an SQL `IN (...)` predicate for PostgreSQL and MSSQL.

#### Scenario: Several predefined catalog values
- **WHEN** a predicate uses
  `В (ЗНАЧЕНИЕ(Справочник.бит_СтатусыОбъектов.Утвержден), ЗНАЧЕНИЕ(Справочник.бит_СтатусыОбъектов.ДополнительныеУсловияПоДоговору_Проверен))`
- **THEN** generated SQL compares the left operand with both resolved catalog
  `_IDRRef` lookup expressions in the same order

#### Scenario: English alias and typed literals
- **WHEN** a predicate uses `IN` with one or more scalar literals
- **THEN** each literal uses the target dialect and the left field's resolved
  physical type

#### Scenario: Empty or malformed list
- **WHEN** `В`/`IN` is followed by an empty list, a trailing comma, or no closing
  parenthesis
- **THEN** compilation returns a positional diagnostic and emits no SQL

### Requirement: Alias projected query columns
The `open-sdbl` query compiler SHALL accept an explicit `КАК`/`AS` alias after
each supported projection expression and SHALL expose that alias as the stable
logical result label in PostgreSQL and MSSQL output.

#### Scenario: Field and dereference aliases
- **WHEN** direct and one-hop reference-property projections are followed by
  explicit aliases
- **THEN** generated SQL and `CompiledQuery.columns` use the requested aliases
  instead of the underlying field-path labels

#### Scenario: Compound projection alias
- **WHEN** an explicitly aliased logical field expands into several physical
  SQL columns
- **THEN** every member receives a unique deterministic label derived from the
  requested alias and its existing compound-member suffix

#### Scenario: Missing projection alias
- **WHEN** `КАК`/`AS` is not followed by a contextual identifier
- **THEN** compilation returns a positional diagnostic and no SQL is produced

### Requirement: Query authoritative tabular sections
The `open-sdbl` query compiler SHALL accept a document or catalog source shaped
as `<kind>.<object>.<section>`. It SHALL resolve the parent object through
DBNames and Config, resolve the section by a nested Config descriptor and its
exact DBNames `VT` entry, and require the resulting
`<parent-physical-table>_VT<number>` or its exact configuration-extension
`X[digits]` variants in SchemaStorage and the live catalog.

#### Scenario: Joined document tabular section
- **WHEN** a document tabular section is joined to another metadata source by
  its `Ссылка` field, the opposing field is a compound reference, and fields
  are selected with explicit aliases
- **THEN** generated SQL reads the exact tabular-section table, uses its owner
  reference in the JOIN, compares both the reference payload and the
  authoritative target-type discriminator, and preserves the requested output
  labels

#### Scenario: Reference property from a tabular section
- **WHEN** a tabular-section field or owner reference has one authoritative
  SchemaStorage reference target and a target property is selected
- **THEN** the existing reusable one-hop LEFT JOIN resolves that property

#### Scenario: Extended tabular-section storage
- **WHEN** the canonical tabular-section table is absent and SchemaStorage plus
  the live catalog contain exact `X[digits]` variants
- **THEN** fields are resolved from an authoritative variant and generated SQL
  reads all exact variants through one deterministic relation

#### Scenario: Inline SchemaStorage declaration
- **WHEN** SchemaStorage declares a section as
  `{"VT<number>","I",0,"<parent>",...}`
- **THEN** metadata resolution exposes the canonical
  `<parent>_VT<number>` table, its declared columns, and the implied
  `<parent>_IDRRef` owner reference

#### Scenario: Standard tabular-section fields
- **WHEN** the section table declares its parent reference and numbered line
  field
- **THEN** they are queryable as `Ссылка`/`ID` and
  `НомерСтроки`/`LineNo` respectively

#### Scenario: Invalid tabular-section mapping
- **WHEN** the parent, nested descriptor, exact `VT` entry, SchemaStorage table,
  or live table is missing or ambiguous
- **THEN** compilation returns a specific diagnostic without guessing from
  similarly prefixed physical tables

### Requirement: Compile metadata value expressions

The compiler SHALL accept `ЗНАЧЕНИЕ`/`VALUE` with exactly one
`<kind>.<object>.<value>` metadata path for catalog and enumeration kinds. It
SHALL resolve the object and value through the metadata snapshot and permit the
expression in projections and predicates.

#### Scenario: Enumeration value
- **WHEN** a query uses
  `ЗНАЧЕНИЕ(Перечисление.бит_ВидыСтатусовОбъектов.Статус)`
- **THEN** PostgreSQL and MSSQL SQL contain the enumeration GUID in physical 1C
  byte order as a typed binary expression

#### Scenario: Catalog predefined value
- **WHEN** a query uses
  `ЗНАЧЕНИЕ(Справочник.бит_СтатусыОбъектов.Утвержден)`
- **THEN** generated SQL returns `_IDRRef` from the resolved catalog table by
  equality on `_PredefinedID` and the stable metadata GUID

#### Scenario: Hierarchical symbolic name
- **WHEN** a catalog value name contains underscores such as
  `ДополнительныеУсловияПоДоговору_Проверен`
- **THEN** the complete symbolic name is resolved exactly as one path component
  without splitting or inspecting presentation data

#### Scenario: Invalid value expression
- **WHEN** the path shape, kind, object, value, live table, or required physical
  columns are unsupported, absent, or ambiguous
- **THEN** compilation returns a positional diagnostic and emits no SQL

### Requirement: Detect the MSSQL dialect level at connection
When connecting to Microsoft SQL Server the CLI SHALL read
`SERVERPROPERTY('ProductVersion')`, map major versions below 11 to the
`Sql2008` level and 11 or above to `Sql2012`, print the chosen level and the
server version, and use that level for every compiled query. The
`--mssql-dialect 2008|2012` option SHALL override detection and SHALL be
rejected for other providers or values.

#### Scenario: SQL Server 2008 R2
- **WHEN** the server reports product version `10.50.6000.34`
- **THEN** the console prints the `2008` level and `НАЧАЛОПЕРИОДА` queries
  execute without error 195

#### Scenario: Explicit override
- **WHEN** the user passes `--mssql-dialect 2008` against SQL Server 2019
- **THEN** the console compiles with the `Sql2008` level and the query results
  equal those of the default level

### Requirement: Compile cast expressions
The compiler SHALL accept bilingual `ВЫРАЗИТЬ`/`CAST` with one expression
and a target of `СТРОКА(n)`, `ЧИСЛО(p,s)`, `БУЛЕВО`, `ДАТА`, or a tabular
metadata object. Scalar targets SHALL compile to native conversions on both
providers and report the target as the column kind. A metadata target SHALL
require a reference field, SHALL yield the reference value guarded by the
runtime type discriminator, and MAY be followed by one `.Field` that
dereferences through a type-guarded join reusing the presentation join
cache. Unsupported targets, non-reference arguments, and deeper paths SHALL
fail with positional diagnostics.

#### Scenario: Bounded string
- **WHEN** a query projects `ВЫРАЗИТЬ(Комментарий КАК СТРОКА(500))`
- **THEN** PostgreSQL SQL uses `substring(… from 1 for 500)`, MSSQL SQL uses
  `CONVERT(nvarchar(500), …)`, and the column kind is a string of length 500

#### Scenario: Narrowed dereference
- **WHEN** a query projects `ВЫРАЗИТЬ(Регистратор КАК Документ.Заказ).Дата`
  from a universal reference field
- **THEN** generated SQL joins the document table on `RRRef` with an `RTRef`
  guard and projects its date column

#### Scenario: Fixed target mismatch
- **WHEN** a field references exactly one catalog and the cast names a
  different object
- **THEN** compilation fails with a positional diagnostic

### Requirement: Render boolean predicates on MSSQL
When a boolean field, boolean literal, or boolean cast appears in a predicate
position (`WHERE`, `ON`, or an operand of `AND`, `OR`, `NOT`), generated
T-SQL SHALL compare it with `0x01`/`0x00` so that SQL Server accepts it;
PostgreSQL SQL SHALL keep the bare boolean form.

#### Scenario: Bare boolean field
- **WHEN** a query filters with `ГДЕ Проведен И Код = "A"`
- **THEN** MSSQL SQL contains `([t].[_posted] = 0x01)` and PostgreSQL SQL
  contains the bare column

### Requirement: Compile conditional expressions
The compiler SHALL accept `ВЫБОР КОГДА <predicate> ТОГДА <value> …
[ИНАЧЕ <value>] КОНЕЦ` / `CASE WHEN … THEN … [ELSE …] END` wherever a scalar
expression is allowed and SHALL render it as SQL `CASE` on both providers,
compiling each condition as a predicate. The column kind SHALL be the first
non-wildcard branch kind and every branch kind SHALL be compatible with it,
otherwise compilation SHALL fail with a positional diagnostic. Reference
branches with different targets or widths SHALL be widened to one
runtime-typed payload whose targets are the union of the branch targets. An absent `ИНАЧЕ` SHALL
yield `NULL`.

#### Scenario: Conditional projection
- **WHEN** a query projects `ВЫБОР КОГДА Проведен ТОГДА "Да" ИНАЧЕ "Нет" КОНЕЦ`
- **THEN** both dialects emit `CASE WHEN … THEN … ELSE … END`, MSSQL compares
  the boolean field with `0x01` in the condition, and the column kind is string

#### Scenario: Conditional predicate on MSSQL
- **WHEN** `ГДЕ ВЫБОР КОГДА Сумма > 0 ТОГДА ИСТИНА ИНАЧЕ ЛОЖЬ КОНЕЦ` is
  compiled for MSSQL
- **THEN** the generated predicate compares the `CASE` expression with `0x01`

#### Scenario: References to different objects
- **WHEN** one branch yields a document reference and another a catalog
  reference
- **THEN** both branches are rendered as `RTRef ‖ RRRef` payloads and the
  column kind is a runtime-typed reference targeting both objects

#### Scenario: Incompatible branches
- **WHEN** one branch yields a number and another a string
- **THEN** compilation fails with a positional diagnostic at the second branch

### Requirement: Compile default-value expressions
The compiler SHALL accept bilingual `ЕСТЬNULL(<value>, <fallback>)` /
`ISNULL(…)` and SHALL render it as `COALESCE(value, fallback)` on both
providers. The column kind SHALL be the value's kind, or the fallback's kind
when the value is the `NULL` literal, and the two kinds SHALL be compatible;
reference operands SHALL be widened by the same rule as `ВЫБОР` branches.

#### Scenario: Default after an outer join
- **WHEN** a query projects `ЕСТЬNULL(Остатки.КоличествоОстаток, 0)` over a
  left join
- **THEN** generated SQL contains `COALESCE(…, 0)` and the column kind is
  number

#### Scenario: Date default on MSSQL
- **WHEN** `ЕСТЬNULL(Дата, ДАТАВРЕМЯ(1, 1, 1))` is projected with a non-zero
  year offset
- **THEN** the `DATEADD(year, -offset, …)` correction wraps the whole
  `COALESCE` exactly once

### Requirement: Compile pattern predicates
The compiler SHALL accept `<value> [НЕ] ПОДОБНО <pattern> [СПЕЦСИМВОЛ
<escape>]` / `[NOT] LIKE … [ESCAPE …]` in predicate positions and SHALL
render `LIKE` with the pattern passed through unchanged on both providers,
adding `ESCAPE` when supplied and `NOT` when negated. The operands SHALL have
the string kind or a wildcard kind. Using the operator as a projected value
or with non-string operands SHALL fail with a positional diagnostic.

#### Scenario: Bracket class pattern
- **WHEN** a query filters with `Код ПОДОБНО "[0-9]%"`
- **THEN** PostgreSQL and MSSQL SQL both contain `LIKE` with the literal
  pattern `[0-9]%` and no rewritten pattern

#### Scenario: Negated pattern with an escape character
- **WHEN** a query filters with `Наименование НЕ ПОДОБНО "%\_%" СПЕЦСИМВОЛ "\"`
- **THEN** generated SQL wraps the `LIKE … ESCAPE …` predicate in `NOT (…)`

### Requirement: Aggregate arbitrary scalar expressions
`СУММА`, `СРЕДНЕЕ`, `МИНИМУМ`, `МАКСИМУМ`, and `КОЛИЧЕСТВО([РАЗЛИЧНЫЕ] …)`
SHALL accept any scalar expression as their argument. `СУММА`, `СРЕДНЕЕ`,
and `КОЛИЧЕСТВО` SHALL report a number kind; `МИНИМУМ`/`МАКСИМУМ` SHALL
report the argument's kind and SHALL aggregate the payload of a reference
expression on both providers. `СРЕДНЕЕ` SHALL render as `AVG` and SHALL
refuse `РАЗЛИЧНЫЕ` and `*` as `СУММА` does. Aggregates nested in aggregates
SHALL fail with a positional diagnostic.

#### Scenario: Conditional sum
- **WHEN** a query projects `СУММА(ВЫБОР КОГДА Вид = ЗНАЧЕНИЕ(…) ТОГДА Сумма ИНАЧЕ 0 КОНЕЦ)`
- **THEN** generated SQL contains `SUM(CASE WHEN … END)` and the column kind
  is number

#### Scenario: Distinct count of an expression
- **WHEN** a query projects `КОЛИЧЕСТВО(РАЗЛИЧНЫЕ НАЧАЛОПЕРИОДА(Дата, МЕСЯЦ))`
- **THEN** generated SQL contains `COUNT(DISTINCT …)` over the period
  expression

#### Scenario: Grouped average
- **WHEN** a query projects `СРЕДНЕЕ(Т.Цена * Т.Количество) КАК Среднее`
  in a grouped branch
- **THEN** generated SQL contains `AVG(…)` over the product and the column
  kind is number

### Requirement: Compile named query parameters
The compiler SHALL accept `&Имя` wherever a scalar expression is allowed and
SHALL inline the supplied `ParameterValue` as a typed literal of the target
dialect: `NULL`, booleans, decimal numbers from an unscaled integer and
scale, strings, dates shifted by the MSSQL year offset, 16-byte references
guarded by `RTRef` when compared with runtime-typed fields, raw binary, and
lists inside `В`/`IN` (an empty list SHALL compile to an always-false
predicate; `NULL` elements SHALL be rendered as `NULL`). Scalar parameters
SHALL be rendered for the other operand's column type without a kind check,
as written literals are. Parameter names SHALL be matched case-insensitively. Date
parameters SHALL be accepted as virtual-table periods and any parameter
SHALL be accepted inside virtual-table conditions. The column kind of a
parameter SHALL follow its value.

#### Scenario: Date and reference parameters
- **WHEN** `ГДЕ Дата >= &Начало И Склад = &Склад` is compiled with a date and
  a catalog reference value for MSSQL with a non-zero year offset
- **THEN** generated SQL contains the shifted date literal and the 16-byte
  binary literal compared with the warehouse `RRRef`

#### Scenario: List parameter
- **WHEN** `Склад В (&Склады)` is compiled with a list of two references
- **THEN** generated SQL contains `IN (0x…, 0x…)` in list order

#### Scenario: Virtual-table period parameter
- **WHEN** `РегистрСведений.Курсы.СрезПоследних(&Период, Валюта = &Валюта)` is
  compiled with a date and a reference value
- **THEN** the slice uses the date literal as its period bound and the
  reference literal in its condition

### Requirement: Compile empty references
The compiler SHALL accept `ЗНАЧЕНИЕ(<Вид>.<Объект>.ПустаяСсылка)` /
`VALUE(<Kind>.<Object>.EmptyRef)` for every metadata kind that has a
reference table and SHALL render it as a 16-byte zero binary literal whose
kind is a fixed reference to that object.

#### Scenario: Empty document reference
- **WHEN** a query filters with `Заказ = ЗНАЧЕНИЕ(Документ.Заказ.ПустаяСсылка)`
- **THEN** generated SQL compares the order `RRRef` with sixteen zero bytes
  and the expression kind is a reference to the order document

### Requirement: Compare references with output-format binary values
The compiler SHALL accept a binary literal or a `Binary` parameter as the
other operand of `=`, `<>`, or `В`/`IN` against a reference field when its
length matches the field's output format: 16 bytes for a single-member
reference and 20 bytes (`RTRef ‖ RRRef`) for a runtime-typed field, in which
case the comparison SHALL be split into member comparisons. Any other length
SHALL fail with a positional diagnostic naming the expected form.

#### Scenario: Pasting a printed recorder
- **WHEN** a query filters with `Регистратор = 0x<40 hex digits>` copied from
  the console output of a runtime-typed field
- **THEN** generated SQL compares `_RTRef` with the first four bytes and
  `_RRRef` with the remaining sixteen

#### Scenario: Wrong width
- **WHEN** a 16-byte literal is compared with a runtime-typed field
- **THEN** compilation fails with a positional diagnostic that names the
  20-byte form

### Requirement: Manage console query parameters
The console SHALL provide `\set <Имя> <литерал>` to store a session
parameter from an SDBL literal (number, string, `ИСТИНА`/`ЛОЖЬ`, `NULL`,
`ДАТАВРЕМЯ(…)`, `0x…`, `ЗНАЧЕНИЕ` of an enumeration value or an empty
reference, or a parenthesized list of those), `\params` to list stored
parameters with their original literal text and value kind, and
`\unset <Имя>` to remove one; `\set` without arguments SHALL print the
command syntax. When executing a query the
console SHALL pass only the parameters the query references and SHALL
surface the compiler's parameter diagnostics unchanged.

#### Scenario: Stored parameter used by a query
- **WHEN** the user enters `\set Период ДАТАВРЕМЯ(2024, 1, 1)` and then a
  query referencing `&Период`
- **THEN** the query compiles with the stored date and the generated SQL
  contains the date literal

#### Scenario: Listing parameters
- **WHEN** the user enters `\params` after setting a date and a list
- **THEN** the console prints one line per parameter with its name, the
  literal as entered, and its kind

#### Scenario: Stale parameter
- **WHEN** a stored parameter is not referenced by the next query
- **THEN** the query compiles without an unused-parameter diagnostic

#### Scenario: Missing parameter
- **WHEN** a query references a parameter that was never set
- **THEN** the console prints the compiler's positional parameter diagnostic
  and remains available

### Requirement: Compile grouped branches
The compiler SHALL accept `СГРУППИРОВАТЬ ПО <keys>` / `GROUP BY` after the
filter of a branch and an optional `ИМЕЮЩИЕ <predicate>` / `HAVING` after
it. A key SHALL be a one-hop field path, a projection alias, or an
expression textually equal to a projected expression. Every non-aggregate
projection SHALL match a key, otherwise compilation SHALL fail with a
positional diagnostic. Generated SQL SHALL group by every physical member of
a reference key and by the physical columns of inline presentations of
keys, SHALL compile `ИМЕЮЩИЕ` as a predicate that may contain aggregates, SHALL
accept aggregates inside `ВЫБОР` branches of grouped projections,
SHALL reject aggregates in `ГДЕ`, `ПО`, and keys, SHALL restrict grouped
ordering to keys and projection aliases, and SHALL reject grouping combined
with `ПОЛНОЕ СОЕДИНЕНИЕ`.

#### Scenario: Grouped sum over a reference key
- **WHEN** a query projects `Номенклатура, СУММА(Количество)` and groups by
  `Номенклатура`
- **THEN** both dialects emit `GROUP BY` over the `_RTRef` and `_RRRef`
  members of the field and project the one-column reference payload

#### Scenario: Ungrouped projection
- **WHEN** a grouped query projects a field that is neither aggregated nor a
  key
- **THEN** compilation fails with a positional diagnostic at that field

#### Scenario: Having predicate
- **WHEN** a query groups by `Склад` and filters with
  `ИМЕЮЩИЕ СУММА(Количество) > 100`
- **THEN** generated SQL contains `HAVING SUM(…) > 100` and no aggregate in
  `WHERE`

#### Scenario: Aggregate inside a conditional projection
- **WHEN** a grouped query projects
  `ВЫБОР КОГДА СУММА(Количество) > 0 ТОГДА "Есть" ИНАЧЕ "Нет" КОНЕЦ`
- **THEN** generated SQL contains the `CASE` with `SUM(…)` in its condition

#### Scenario: Dereferenced key with a presentation
- **WHEN** a query groups by `Номенклатура.Родитель` and projects
  `ПРЕДСТАВЛЕНИЕ(Номенклатура.Родитель)`
- **THEN** generated SQL joins the parent through the shared reference join
  and groups by the joined key and presentation columns

### Requirement: Compile nested query sources
The compiler SHALL accept `(<query>) [КАК] <alias>` as a source in `ИЗ` and
in any join, compiling the nested query as an independent statement whose
output columns become the derived source's fields with their column kinds.
A nested query MAY use unions, grouping, joins, `ПЕРВЫЕ`, `РАЗЛИЧНЫЕ`,
inline presentations, and final ordering when every branch has `ПЕРВЫЕ`,
but SHALL NOT contain ordering over a branch without `ПЕРВЫЕ`, deferred
reference presentations, or `*`. The ordering of a nested union SHALL be
dropped: each branch limits itself and the union has no limit of its own,
so the order changes nothing. A
derived column whose kind is a fixed single-target reference SHALL support
one-hop dereference through the shared join cache; a runtime-typed derived
column SHALL be dereferenced under the composite-reference rules with its
known targets as the declared candidates. Identifiers that resolve only in an enclosing query SHALL
fail with a positional diagnostic. Nested statements SHALL count toward the
parser depth limit and the work budget.

#### Scenario: Grouped nested source joined to a catalog
- **WHEN** a query joins `(ВЫБРАТЬ Номенклатура, СУММА(Количество) КАК Итог ИЗ … СГРУППИРОВАТЬ ПО Номенклатура) КАК Т`
  to `Справочник.Номенклатура` on `Т.Номенклатура = Н.Ссылка`
- **THEN** both dialects emit the nested statement as a parenthesized derived
  table with the alias, and `Т.Итог` is projected as a number column

#### Scenario: Dereference through a derived reference
- **WHEN** the outer query projects `Т.Номенклатура.Наименование`
- **THEN** generated SQL left-joins the catalog on the derived reference
  column and projects its description

#### Scenario: First N as a source
- **WHEN** a nested source is `ВЫБРАТЬ ПЕРВЫЕ 10 Ссылка, Дата ИЗ Документ.Заказ УПОРЯДОЧИТЬ ПО Дата УБЫВ`
- **THEN** MSSQL emits `TOP (10) … ORDER BY` and PostgreSQL emits
  `ORDER BY … LIMIT 10` inside the derived table

#### Scenario: Correlated reference
- **WHEN** a nested query's filter names a field of the outer source
- **THEN** compilation fails with a positional diagnostic at that field

#### Scenario: Nested union of limited branches
- **WHEN** `ВЫБРАТЬ ПЕРВЫЕ 10 Д.Ссылка КАК Ссылка ПОМЕСТИТЬ ВТ ИЗ Документ.X КАК Д ОБЪЕДИНИТЬ ВСЕ ВЫБРАТЬ ПЕРВЫЕ 10 Д.Ссылка ИЗ Документ.Y КАК Д УПОРЯДОЧИТЬ ПО Ссылка;`
  is compiled
- **THEN** the definition compiles with a limit on each branch and no
  `ORDER BY`

#### Scenario: Nested union with an unlimited branch
- **WHEN** one branch of an ordered nested union has no `ПЕРВЫЕ`
- **THEN** compilation fails with a positional diagnostic

### Requirement: Compile subquery membership predicates
The compiler SHALL accept `<expr> [НЕ] В (<query>)` / `[NOT] IN (SELECT …)`
where the nested query projects exactly one column of a kind compatible
with the left operand, and `<expr> НЕ В (<list>)`. Scalar operands SHALL
compile to `[NOT] IN (…)`. Reference operands SHALL compare `RRRef`
members: a runtime-typed left operand against a fixed-target subquery SHALL
add an `RTRef` guard, a fixed-target left operand against a runtime-typed
subquery SHALL filter the subquery by that `RTRef`, and two runtime-typed
sides SHALL compare payloads. A subquery with more than one column SHALL fail
with a positional diagnostic.

#### Scenario: Reference membership
- **WHEN** a query filters with `Номенклатура В (ВЫБРАТЬ Ссылка ИЗ Справочник.Номенклатура ГДЕ ПометкаУдаления)`
  on a fixed-target field
- **THEN** generated SQL compares the field's `RRRef` with `IN (SELECT …)`
  over the catalog `_IDRRef`

#### Scenario: Guarded membership
- **WHEN** the left operand is a runtime-typed reference and the subquery
  projects catalog references
- **THEN** generated SQL wraps the `IN` predicate with an `RTRef` equality
  for that catalog

#### Scenario: Negated list
- **WHEN** a query filters with `Код НЕ В ("1", "2")`
- **THEN** generated SQL contains `NOT IN ('1', '2')`

### Requirement: Compile temporary-table batches
The compiler SHALL accept a batch of statements separated by `;`. A
statement SHALL be a query optionally carrying `ПОМЕСТИТЬ <Имя>` / `INTO`
or `ДОБАВИТЬ <Имя>` / `ADD` after its first selection list and optionally
ending with `ИНДЕКСИРОВАТЬ ПО <поля>` / `INDEX BY`, `ИНДЕКСИРОВАТЬ ПО
НАБОРАМ ((…), …)` / `INDEX BY SETS`, each with an optional `УНИКАЛЬНО` /
`UNIQUE`, or the statement `УНИЧТОЖИТЬ <Имя>` / `DROP`. Temporary tables
SHALL be emulated with common table expressions named `vt1`, `vt2`, … in
definition order: `ПОМЕСТИТЬ` SHALL define a new CTE from the statement
compiled under the nested-query rules (no `*`, no deferred presentations,
ordering only with `ПЕРВЫЕ`), `ДОБАВИТЬ` SHALL define a new CTE
`SELECT … FROM <previous> UNION ALL <statement>` and rebind the name after
a strict positional structure check (equal column count, compatible kinds,
identical reference targets and width, `NULL` compatible with anything),
and `УНИЧТОЖИТЬ` SHALL emit nothing and hide the name so that it MAY be
defined again. `ИНДЕКСИРОВАТЬ ПО` fields SHALL name output labels of the
statement and SHALL generate nothing. A bare identifier source `ИЗ <Имя>
[КАК <Псевдоним>]`, also in joins, nested queries, and `В (ВЫБРАТЬ …)`,
SHALL read a visible temporary table as a derived source whose fields carry
the stored columns and kinds, with the table name as the default alias.
The batch SHALL compile to one statement whose `WITH` list contains
exactly the CTEs reachable from the final statement in ascending order; a
final `ПОМЕСТИТЬ` or `ДОБАВИТЬ` SHALL yield one `Количество` row counting
the rows placed or appended, and a final `УНИЧТОЖИТЬ` SHALL yield no
statement. Temporary-table definitions SHALL stay in the storage date
domain so that the final projection corrects MSSQL dates once. A batch of
more than 64 statements SHALL fail with `WorkBudgetExceeded`, and each
statement SHALL charge the work budget.

#### Scenario: Define and read a temporary table
- **WHEN** a batch is `ВЫБРАТЬ Ссылка КАК Товар, СУММА(Количество) КАК Итог ПОМЕСТИТЬ Обороты ИЗ … СГРУППИРОВАТЬ ПО Ссылка; ВЫБРАТЬ Т.Товар, Т.Итог ИЗ Обороты КАК Т ГДЕ Т.Итог > 0`
- **THEN** both dialects emit `WITH "vt1" AS (…) SELECT … FROM "vt1" AS "Т" WHERE …`
  with the dialect's identifier quoting and the columns `Товар` (reference)
  and `Итог` (number)

#### Scenario: Append rows
- **WHEN** a second statement is `ВЫБРАТЬ Наименование КАК Наименование ДОБАВИТЬ ВТ ИЗ Справочник.Услуги`
  after `ВТ` was placed from `Справочник.Товары` with one string column
- **THEN** the batch defines `vt2 AS (SELECT "Наименование" FROM "vt1" UNION ALL SELECT … )`,
  later reads of `ВТ` use `vt2`, and the `WITH` list carries `vt1` before
  `vt2`

#### Scenario: Structure mismatch on append
- **WHEN** `ДОБАВИТЬ` appends two columns to a one-column table or a
  reference column of another target
- **THEN** compilation fails with a `TemporaryTable` diagnostic at the
  `ДОБАВИТЬ` token and no SQL is produced

#### Scenario: Placement count
- **WHEN** the batch ends with a `ПОМЕСТИТЬ` statement
- **THEN** the generated statement is `WITH … SELECT COUNT(*) AS "Количество" FROM "vtN"`
  with one number column labelled `Количество`

#### Scenario: Drop and redefine
- **WHEN** a batch drops `ВТ` and then places `ВТ` again
- **THEN** the second definition succeeds as a new CTE, a read between the
  drop and the redefinition fails with a `TemporaryTable` diagnostic, and a
  batch ending with the drop yields no statement

#### Scenario: Unreachable definition omitted
- **WHEN** the final statement reads only `ВТ2` while `ВТ1` was placed
  earlier and is not referenced by `ВТ2`
- **THEN** the `WITH` list contains only the CTE of `ВТ2`

#### Scenario: Index clause ignored
- **WHEN** a definition ends with `ИНДЕКСИРОВАТЬ ПО НАБОРАМ ((Код, Наименование) УНИКАЛЬНО, (Артикул))`
- **THEN** the batch compiles without any index SQL, and a field absent
  from the selection list fails with a `TemporaryTable` diagnostic at that
  field

#### Scenario: Definition restrictions
- **WHEN** a `ПОМЕСТИТЬ` statement projects `*`, requests a deferred
  presentation, orders without `ПЕРВЫЕ`, or appears inside a nested query
- **THEN** compilation fails with a positional diagnostic

### Requirement: Manage temporary tables across compilations
The `open-sdbl` library SHALL provide `TempTablesManager`, a plain value
holding compiled temporary-table definitions (name, CTE name, SQL,
columns, dependencies, visibility) that survives between compilations.
`QueryCompiler::compile_batch(source, &options, &mut manager)` and
`Prepared::compile_batch(snapshot, &options, &mut manager)` SHALL read
visible tables from the manager, SHALL return `Ok(None)` when the batch
ends with `УНИЧТОЖИТЬ`, and SHALL commit the batch's definitions and drops
to the manager only when compilation succeeds.
`QueryCompiler::prepare_with(source, &manager)` SHALL collect presentation
targets with the manager's tables visible. The manager SHALL expose the
visible tables with their names and `CompiledColumn`s, `contains`,
`is_empty`, and `clear`. The first definition SHALL bind the manager to the
compiling dialect and snapshot; use with another dialect SHALL fail with
`TemporaryTable` and with another snapshot with `SnapshotMismatch`. A
manager SHALL hold at most 256 definitions. `compile`, `compile_with`,
`prepare`, `compile_with_presentations`, and `Prepared::compile` SHALL
accept batches with a manager private to the call and SHALL fail with a
`TemporaryTable` diagnostic when the batch returns no rows.

#### Scenario: Definition reused by a later batch
- **WHEN** an application compiles `ВЫБРАТЬ … ПОМЕСТИТЬ ВТ ИЗ … ГДЕ Дата > &Период`
  with a bound `Период` and then compiles `ВЫБРАТЬ Т.Дата ИЗ ВТ КАК Т`
  with the same manager and no parameters
- **THEN** the second SQL contains the first definition with the inlined
  date literal and no unused-parameter diagnostic is raised

#### Scenario: Failed batch leaves the manager unchanged
- **WHEN** a batch places `ВТ1` and then fails on its second statement
- **THEN** the manager still lists the tables it had before the call

#### Scenario: Manager listing
- **WHEN** an application iterates `manager.tables()` after placing and
  appending to `ВТ`
- **THEN** it sees one entry `ВТ` with the columns and kinds of the first
  definition

#### Scenario: Batch without rows through compile
- **WHEN** an application calls `compile` on `УНИЧТОЖИТЬ ВТ`
- **THEN** compilation fails with a `TemporaryTable` diagnostic

### Requirement: Manage console temporary tables
The console SHALL keep one `TempTablesManager` for the session, SHALL run
every statement through `prepare_with` and `Prepared::compile_batch`, SHALL
execute and display the statement when one is produced (so a `ПОМЕСТИТЬ`
statement shows its `Количество` row), SHALL print the names dropped from
the manager when none is produced, SHALL provide `\tables` listing every
visible temporary table with its name and columns as label and kind, SHALL
clear the manager with a notice on `\refresh`, and SHALL offer visible
table names in source completion.

#### Scenario: Batch entered statement by statement
- **WHEN** the user enters a `ПОМЕСТИТЬ` statement terminated by `;` and
  then a query reading that table terminated by `;`
- **THEN** the first shows a `Количество` row, the second executes the
  `WITH` statement, and `\tables` lists the table in between

#### Scenario: Drop notice
- **WHEN** the user enters `УНИЧТОЖИТЬ ВТ;`
- **THEN** the console prints that `ВТ` was dropped, executes no SQL, and
  `\tables` no longer lists it

#### Scenario: Refresh clears definitions
- **WHEN** the user enters `\refresh` after placing a table
- **THEN** the console reloads metadata, prints that temporary tables were
  cleared, and `\tables` reports none

### Requirement: Widen reference equalities in join conditions
A join equality whose operands are references of different width SHALL
compare `RTRef ‖ RRRef` payloads: a fixed single-target reference SHALL be
widened to its target type number followed by its identifier, a composite
field SHALL be widened to the concatenation of its `_RTRef` and `_RRRef`
members, and a runtime-typed column of a derived source or temporary table
SHALL be used as it is. Operands of equal width SHALL keep their direct
comparison. The widened expression SHALL serve as the join anchor marker.

#### Scenario: Fixed reference joined to a temporary-table column
- **WHEN** a catalog is joined to a temporary table on `Д.Ссылка = П.Ссылка`
  where `П.Ссылка` was placed from a composite register field
- **THEN** both dialects compare the catalog's type number concatenated
  with `_IDRRef` against the payload column, and the join matches rows

#### Scenario: Composite field joined to a derived column
- **WHEN** a register's composite `Объект` is joined to a grouped nested
  source's `Документ` column that was projected from the same composite field
- **THEN** generated SQL compares `_RTRef ‖ _RRRef` with the derived column
  instead of failing with a missing-target diagnostic

#### Scenario: Payload against payload
- **WHEN** two temporary tables are joined on runtime-typed columns
- **THEN** generated SQL compares the two columns directly

### Requirement: Dereference references inside join conditions
The compiler SHALL accept one-hop dereferences through fixed single-target
references of the joined source and of earlier sources anywhere in a join
condition. When a condition uses a dereference, the generated SQL SHALL
render the dereference joins of every source as a parenthesized group next
to that source's relation so that every `ON` clause can reference them;
statements whose conditions use no dereference SHALL keep the flat join
list. A `ПОЛНОЕ [ВНЕШНЕЕ] СОЕДИНЕНИЕ` whose condition dereferences a
reference SHALL fail with a positional diagnostic.

#### Scenario: Tabular section joined through its owner
- **WHEN** a query joins `Документ.Корреспонденция.Корреспонденты КАК К`
  to `Справочник.ДокументыПредприятия КАК Д` on `К.Ссылка.Основание = Д.Ссылка`
- **THEN** both dialects emit the tabular section grouped with a `LEFT JOIN`
  of its owner document and compare the owner's `Основание` column with the
  catalog identifier in the `ON` clause

#### Scenario: Earlier source dereferenced in a later condition
- **WHEN** the third source of a chain is joined on a property of the first
  source's reference field
- **THEN** the first source is rendered as a group with its dereference
  join and the third `ON` references that join's alias

#### Scenario: Flat rendering preserved
- **WHEN** no join condition dereferences a reference
- **THEN** the generated SQL is identical to the SQL generated before this
  change

### Requirement: Dereference composite reference fields
The compiler SHALL accept a one-hop dereference through a composite
reference field (`_RTRef` and `_RRRef` members) and through a runtime-typed
column of a derived source or temporary table. Candidate targets SHALL be
the field's declared SchemaStorage targets when present, otherwise every
reference-kind metadata object whose fields include the named attribute.
Candidates without the attribute SHALL be skipped; no candidate SHALL fail
with `UnknownField`; more candidates than the statement can carry — 256,
the number of tables SQL Server accepts in one statement — SHALL fail with
`UnsupportedFeature` naming `ВЫРАЗИТЬ`. Each candidate SHALL be joined with
a `LEFT JOIN` guarded by its type number through the shared join key, and
the value SHALL be a `CASE` over the reference type selecting the
candidate's column, yielding `NULL` for rows of other types. The result
kind SHALL be the common kind of the attribute across candidates: equal
variants (else a positional diagnostic), the widest string length, number
without precision, references widened to a runtime-typed payload with the
union of targets. Presentation of the value and a second hop SHALL fail
with `UnsupportedFeature`.

#### Scenario: Any-reference field dereferenced by attribute scan
- **WHEN** a query projects `Связь.СвязанныйОбъект.РегистрационныйНомер`
  from a register whose `СвязанныйОбъект` declares no targets and two
  catalogs define `РегистрационныйНомер`
- **THEN** both dialects left-join each catalog on `_RRRef` with an `_RTRef`
  type guard and project `CASE WHEN _RTRef = <type1> THEN … WHEN _RTRef =
  <type2> THEN … END` as a string column

#### Scenario: Declared multi-target field
- **WHEN** a query filters on `Регистратор.Номер` of a register whose
  recorder declares three document targets
- **THEN** only the declared documents are joined, each with its type
  guard, and the filter compares the `CASE` value

#### Scenario: Temporary-table payload column
- **WHEN** a temporary table placed from a composite field is read with
  `Т.Ссылка.Наименование`
- **THEN** the payload column is split into its type and identifier parts
  for the guarded joins and the `CASE` value

#### Scenario: Many candidates
- **WHEN** an any-reference field of a real configuration is dereferenced
  to an attribute defined by 94 objects
- **THEN** every candidate is joined and the server plans the statement,
  as the platform answers such a query

#### Scenario: Too many candidates
- **WHEN** an any-reference field is dereferenced to an attribute defined
  by more objects than one statement can join
- **THEN** compilation fails with an `UnsupportedFeature` diagnostic that
  suggests narrowing the field with `ВЫРАЗИТЬ`

### Requirement: Manage console session parameters
The console SHALL provide `\session <Имя> [=] <литерал>` to store a session
parameter from the same SDBL literals `\set` accepts, `\session` to list
stored session parameters with their literal text and value kind, and
`\session clear` to forget them all; `\session` with a malformed argument
SHALL print the command syntax. Every query SHALL be compiled with all
stored session parameters, so an unreferenced session parameter is never
an error, and a `\set` parameter referenced by the statement SHALL take
precedence over a session parameter of the same name.

#### Scenario: Session parameter used by a query
- **WHEN** the user enters `\session ТекущийПользователь = "Иванов"` and
  then a query referencing `&ТекущийПользователь`
- **THEN** the query compiles with the stored string

#### Scenario: Unreferenced session parameter
- **WHEN** a stored session parameter is not referenced by the next query
- **THEN** the query compiles without a diagnostic

### Requirement: Manage console access restrictions
The console SHALL provide `\restrict <Вид.Объект[.ТабличнаяЧасть]>
<условие>` to store an access restriction for one metadata table or
tabular section, resolving the name against the metadata snapshot at entry
time and replacing an earlier restriction of the same target, `\restrict`
to list stored restrictions with their target and condition, and
`\restrict clear` to forget them all. Every restriction SHALL carry its
origin — typed by the operator, or derived from the roles of the current
user — and the listing SHALL mark the derived ones. A restriction the
operator types SHALL replace a derived one of the same target and become
a typed restriction, and a derived restriction SHALL never replace a
typed one. Before compiling a batch the console
SHALL pass only the restrictions whose target the prepared batch requested,
together with the session parameters, and SHALL print `Restriction`
diagnostics like every other compiler diagnostic. Without stored
restrictions a `РАЗРЕШЕННЫЕ` query SHALL run unfiltered.

#### Scenario: Restricted query
- **WHEN** the user enters `\restrict Справочник.Номенклатура Организация =
  &Орг`, a matching `\session Орг …`, and then
  `ВЫБРАТЬ РАЗРЕШЕННЫЕ … ИЗ Справочник.Номенклатура`
- **THEN** the generated SQL wraps the catalog in the restricted derived
  table

#### Scenario: Restriction for an unread table
- **WHEN** a stored restriction names a table the next query does not read
- **THEN** the query compiles without a diagnostic

#### Scenario: Unknown target
- **WHEN** the user enters `\restrict Справочник.Нет Код = "1"`
- **THEN** the console prints the metadata lookup error and stores nothing

#### Scenario: Typed restriction over a derived one
- **WHEN** the operator types `\restrict` for a target `\as` derived
- **THEN** the typed condition replaces it and the listing no longer
  marks that target as derived

### Requirement: Calculate end-of-period values
The compiler SHALL accept `КОНЕЦПЕРИОДА`/`ENDOFPERIOD` with a date
expression and the nine periods of `НАЧАЛОПЕРИОДА` and SHALL render the
last second of the period on PostgreSQL and both MSSQL dialect levels:
the beginning of the next period minus one second, where a ten-day period
ends on the 10th, the 20th, or the last day of the month and a week ends
on Sunday. The result kind SHALL be date.

#### Scenario: End of month
- **WHEN** `КОНЕЦПЕРИОДА(ДАТАВРЕМЯ(2020, 2, 10), МЕСЯЦ)` is compiled
- **THEN** the SQL evaluates to `2020-02-29 23:59:59` on both providers

#### Scenario: End of ten-day period
- **WHEN** `КОНЕЦПЕРИОДА(Дата, ДЕКАДА)` is applied to `2020-06-15` and to
  `2020-02-29`
- **THEN** the results are `2020-06-20 23:59:59` and `2020-02-29 23:59:59`

### Requirement: Shift dates by periods
The compiler SHALL accept `ДОБАВИТЬКДАТЕ`/`DATEADD` with a date
expression, one of the bilingual second, minute, hour, day, week, ten-day,
month, quarter, half-year, or year periods, and a count that is a numeric
expression, field, or parameter. Month-based shifts SHALL clamp to the
last day of the target month. A fractional count SHALL be rounded half
away from zero for second, minute, hour, day, week, and month, and
truncated toward zero for ten-day, quarter, half-year, and year, matching
the platform. The result kind SHALL be date and the count SHALL be a
number-kind, parameter, or unknown-kind expression, otherwise compilation
fails with a `Syntax` diagnostic.

#### Scenario: Month-end clamping
- **WHEN** `ДОБАВИТЬКДАТЕ(ДАТАВРЕМЯ(2020, 1, 31, 23, 59, 59), МЕСЯЦ, 1)`
  is compiled
- **THEN** the SQL evaluates to `2020-02-29 23:59:59` on both providers

#### Scenario: Fractional count
- **WHEN** `ДОБАВИТЬКДАТЕ(Дата, ДЕНЬ, 1.5)` and `ДОБАВИТЬКДАТЕ(Дата, ГОД,
  1.5)` are compiled
- **THEN** the first adds two days and the second adds one year

#### Scenario: Count from a field
- **WHEN** `ДОБАВИТЬКДАТЕ(Т.Дата, ДЕНЬ, Т.Количество)` is compiled for a
  numeric field
- **THEN** generated SQL adds the field's rounded value in days

### Requirement: Compute date differences
The compiler SHALL accept `РАЗНОСТЬДАТ`/`DATEDIFF` with two date
expressions and one of the bilingual second, minute, hour, day, month,
quarter, or year units and SHALL return the number of unit boundaries
crossed from the first date to the second (negative when the second date
is earlier), as SQL Server's `DATEDIFF` counts them; `НЕДЕЛЯ`, `ДЕКАДА`,
and `ПОЛУГОДИЕ` SHALL be `Syntax` diagnostics. Seconds, minutes, and hours
SHALL not overflow 32 bits over the full 1C date range on either provider.
On MSSQL with a non-zero year offset both operands SHALL be shifted to
the logical date before the difference is taken. The result kind SHALL be
number.

#### Scenario: Boundary counting
- **WHEN** `РАЗНОСТЬДАТ(ДАТАВРЕМЯ(2020, 12, 31, 23, 59, 59), ДАТАВРЕМЯ(2021, 1, 1), ДЕНЬ)`
  is compiled
- **THEN** the SQL evaluates to `1` on both providers, and `ГОД` gives `1`

#### Scenario: Seconds over the full range
- **WHEN** `РАЗНОСТЬДАТ(ДАТАВРЕМЯ(1, 1, 1), ДАТАВРЕМЯ(2021, 3, 15, 10, 30, 30), СЕКУНДА)`
  is compiled
- **THEN** the SQL evaluates to `63751401030` on both providers

#### Scenario: Unsupported unit
- **WHEN** `РАЗНОСТЬДАТ(А, Б, НЕДЕЛЯ)` is compiled
- **THEN** compilation fails with a `Syntax` diagnostic at the unit token

### Requirement: Extract date parts
The compiler SHALL accept `ГОД`, `КВАРТАЛ`, `МЕСЯЦ`, `ДЕНЬГОДА`, `ДЕНЬ`,
`НЕДЕЛЯ`, `ДЕНЬНЕДЕЛИ`, `ЧАС`, `МИНУТА`, `СЕКУНДА` and their English
spellings with one date field or date-kind expression and SHALL return an
integer number on PostgreSQL and MSSQL: the calendar year, quarter (1–4),
month, day of year, day of month, hour, minute, and second; `ДЕНЬНЕДЕЛИ`
SHALL be 1 for Monday through 7 for Sunday regardless of the server's
first-weekday setting; `НЕДЕЛЯ` SHALL number weeks as the platform does:
the week containing 1 January is week 1, weeks start on Monday, and
numbering restarts on 1 January. On MSSQL with a non-zero year offset the
parts SHALL be taken from the logical date. A non-date argument SHALL be a
`Syntax` diagnostic. The functions SHALL nest with the other date
functions and SHALL be accepted as `GROUP BY` keys.

#### Scenario: Grouping by year
- **WHEN** `ВЫБРАТЬ ГОД(Т.Дата) КАК Год, КОЛИЧЕСТВО(*) КАК Н ИЗ … СГРУППИРОВАТЬ ПО ГОД(Т.Дата)`
  is compiled
- **THEN** generated SQL projects and groups by the year expression and the
  column kind is number

#### Scenario: Platform week numbering
- **WHEN** `НЕДЕЛЯ(Дата)` is applied to 2021-01-03, 2021-01-04, 2024-12-30,
  and 2025-01-01
- **THEN** the results are 1, 2, 53, and 1 on both providers

#### Scenario: Weekday under a year offset
- **WHEN** `ДЕНЬНЕДЕЛИ(Дата)` and `ГОД(Дата)` are compiled for MSSQL with
  year offset 2000
- **THEN** generated SQL applies `DATEADD(year, -2000, …)` to the column
  before taking the parts

### Requirement: Test reference types with the REFS operator
The compiler SHALL accept `<поле> ССЫЛКА <Вид>.<Объект>` /
`<field> REFS <Kind>.<Object>` as a predicate wherever comparisons are
accepted, the operand being a direct field, a one-hop reference
property, or `ВЫРАЗИТЬ(<поле> КАК <Вид>.<Объект>)` naming the same
target as the operator — such a cast keeps a reference of that type and
turns any other into NULL, so the test is the field's. For a composite reference field it SHALL compare the field's
type member with the target's database type number; for a runtime-typed
nested-query column it SHALL compare the first four payload bytes; for a
fixed-target field of the named table it SHALL be a constant true
predicate, which also holds for the empty reference. A fixed-target
field of another table and a non-reference operand SHALL be `Syntax`
diagnostics, and a source-free statement SHALL refuse the operator.

#### Scenario: Composite attribute
- **WHEN** `ГДЕ Т.Объект ССЫЛКА Справочник.Товары` is compiled for a
  composite attribute
- **THEN** generated SQL compares `Т._Fld<N>_RTRef` with the catalog's
  type number on both providers

#### Scenario: Fixed-target attribute
- **WHEN** `ГДЕ Т.Клиент ССЫЛКА Справочник.Клиенты` is compiled for an
  attribute typed with that catalog only
- **THEN** generated SQL contains a true predicate, and naming another
  catalog fails with a `Syntax` diagnostic

#### Scenario: Value position
- **WHEN** `ВЫБОР КОГДА Т.Объект ССЫЛКА Справочник.Товары ТОГДА 1 ИНАЧЕ 0 КОНЕЦ`
  is compiled
- **THEN** the type test is the `CASE` condition

#### Scenario: Cast operand
- **WHEN** `ГДЕ ВЫРАЗИТЬ(Т.Объект КАК Справочник.Товары) ССЫЛКА Справочник.Товары`
  is compiled for a composite attribute
- **THEN** generated SQL compares the attribute's type member with the
  catalog's type number, as the uncast field would

### Requirement: Compute query totals
The compiler SHALL accept `ИТОГИ [<поле> [КАК Псевдоним], …] ПО [ОБЩИЕ]
[<контрольная точка> [ПЕРИОДАМИ(<период>[, <дата>[, <дата>]])] [КАК
Псевдоним], …]` / `TOTALS … BY [OVERALL] …` at the end of a top-level
statement and SHALL return the rows of the platform's linear traversal:
the overall row when `ОБЩИЕ` is present, then for every value of the
first control point one total row followed by the rows of that group,
recursively for deeper control points. Groups SHALL be ordered by the
first appearance of their value in the statement's ordered result and
detail rows SHALL keep that order inside their group. A total row SHALL
carry the control points of its own and enclosing levels, the totals
fields aggregated over the result columns of its group, and `NULL` in
every other column. A totals field SHALL be an expression over
aggregates of result columns; a bare aggregate targets its argument
column, an expression targets the column its alias names, an alias
naming no result column SHALL be a `Syntax` diagnostic, and a later
field naming the same column wins. `ПЕРИОДАМИ` SHALL be validated (a
date control point, date literals or parameters) and SHALL not alter the
rows. `ИТОГИ` with `ПОМЕСТИТЬ`/`ДОБАВИТЬ` SHALL be a `Syntax` diagnostic,
`ИТОГИ` inside a nested query and a control point that is not a result
column SHALL be `UnsupportedFeature` diagnostics.
`CompileOptions::totals_level(true)` SHALL append a numeric `__level`
column equal to the platform's `Уровень()`; the console SHALL enable it.

#### Scenario: Overall and one level
- **WHEN** `ВЫБРАТЬ Т.Родитель КАК Родитель, Т.Наименование КАК Имя,
  Т.Цена КАК Цена ИЗ … УПОРЯДОЧИТЬ ПО Т.Цена УБЫВ ИТОГИ СУММА(Цена) ПО
  ОБЩИЕ, Родитель` is executed
- **THEN** the first row has `NULL` parent and name and the total price,
  each parent's total row precedes its items, parents appear in the
  order of their most expensive item, and the level column reads `0`,
  `1`, `2`

#### Scenario: Totals without fields
- **WHEN** `ИТОГИ ПО Родитель` is executed
- **THEN** each total row carries the parent and `NULL` in the other
  columns

#### Scenario: Union and TOP inputs
- **WHEN** the statement has `ОБЪЕДИНИТЬ ВСЕ` or `ПЕРВЫЕ 3` and totals
- **THEN** the totals are computed over the union rows, or the three
  selected rows, respectively

#### Scenario: Temporary table
- **WHEN** `ПОМЕСТИТЬ ВТ … ИТОГИ СУММА(Цена) ПО ОБЩИЕ` is compiled
- **THEN** compilation fails with a `Syntax` diagnostic at the `ИТОГИ`
  token

### Requirement: Compute hierarchy totals
The compiler SHALL accept `ИЕРАРХИЯ` and `ТОЛЬКО ИЕРАРХИЯ` on one control
point that is a fixed single-target reference to a hierarchical catalog.
With `ИЕРАРХИЯ` the result SHALL contain, before the rows of each value,
one hierarchy total row per ancestor folder of the values present,
aggregating every row beneath the folder; with `ТОЛЬКО ИЕРАРХИЯ` the rows
SHALL be grouped by the parent folder of the value and hierarchy rows
SHALL appear only for folders above those parents. A hierarchy row SHALL
aggregate the rows keyed by the folder itself and by every descendant. A
folder's level SHALL be its depth in the tree, a group total one deeper
than the row above it (its folder's hierarchy row, or its own hierarchy
row when the value is a folder with rows beneath), a detail row one
deeper than its group, all shifted by one under `ОБЩИЕ`. The parent
lookup SHALL read the catalog's extension tables as well as its base
table.
Sibling folders SHALL be ordered by the first appearance of any row
beneath them in the ordered result. A plain control point on the same
column right before the hierarchical one SHALL be ignored. A second
hierarchical control point SHALL be an `UnsupportedFeature` diagnostic;
a control point without a hierarchical catalog target SHALL be a
`Syntax` diagnostic. The standard fields `ParentID` and `OwnerID` SHALL
also answer to `Родитель`/`Parent` and `Владелец`/`Owner`.

#### Scenario: Hierarchy totals
- **WHEN** `… ПО Товар ИЕРАРХИЯ` is executed over items of folders
  `Мебель` ⊃ {`Стол`, `Кухня` ⊃ {`Табурет`}} ordered by name
- **THEN** the rows are the `Мебель` hierarchy total (level 0), the
  `Кухня` hierarchy total (1), the `Табурет` group total (2) and detail
  (3), then the `Стол` group total (1) and detail (2)

#### Scenario: Only hierarchy
- **WHEN** `… ПО Товар ТОЛЬКО ИЕРАРХИЯ` is executed over the same rows
- **THEN** the rows are the `Мебель` hierarchy total (level 0, all
  three items), the `Кухня` group total (1) with `Табурет` beneath it
  (2), and the `Мебель` group total (1) with `Стол` beneath it (2)

#### Scenario: Parent alias
- **WHEN** a query projects `Т.Родитель` from a hierarchical catalog
- **THEN** it resolves to the `_ParentIDRRef` column

### Requirement: Compile type literals and the value-type function
The compiler SHALL accept `ТИП(Строка | Число | Дата | Булево |
<Вид>.<Объект>)` / `TYPE(String | Number | Date | Boolean | …)` as a
constant of kind `Type` and `ТИПЗНАЧЕНИЯ(<выражение>)` /
`VALUETYPE(…)` as an expression of kind `Type`. A type value SHALL be
the five-byte encoding of `TypeValue`. `ТИПЗНАЧЕНИЯ` of a composite
field SHALL read the `_TYPE` member and, when the tag says the value is
a reference, the table number from the `_RTRef` member or from the
field's single reference target; of a composite field that reached the
query through a derived source, the `_TYPE` column that source projects
next to the reference payload; of a runtime-typed payload without such a
column, the payload prefix; of a field, literal, bound parameter, or
expression of a primitive or fixed reference kind, the constant type of
that kind; and of a `NULL` value or an unbound parameter, the `NULL`
type, so the result is never SQL `NULL`. Comparisons and `В (…)` SHALL
compare the encoded values. `ТИП` of any other argument SHALL be a
`Syntax` diagnostic; `ТИПЗНАЧЕНИЯ` of a UUID, binary, or unclassified
expression SHALL be an `UnsupportedFeature` diagnostic; both work in
source-free statements when the argument does. The console SHALL
render a `Type` column by name: `Null`, `Неопределено`, `Булево`,
`Число`, `Строка`, `Дата`, or `<Вид>.<Имя>` of the referenced object.

#### Scenario: Composite attribute
- **WHEN** `ГДЕ ТИПЗНАЧЕНИЯ(Т.Объект) = ТИП(Справочник.Товары)` is
  compiled for a composite attribute
- **THEN** the predicate compares the encoded type, built from
  `_Fld<N>_TYPE` and `_Fld<N>_RTRef`, with `0x0800000035`

#### Scenario: Projected type
- **WHEN** `ВЫБРАТЬ ТИПЗНАЧЕНИЯ(Т.Объект), ТИПЗНАЧЕНИЯ(Т.Цена), ТИП(Строка)`
  is compiled
- **THEN** the columns are of kind `Type`, the first reads the members,
  the second is `0x0300000000` unless the price is `NULL`, and the third
  is the constant `0x0500000000`

#### Scenario: Rejected argument
- **WHEN** `ТИП(УникальныйИдентификатор)` is compiled
- **THEN** compilation fails with a `Syntax` diagnostic

### Requirement: Compile the undefined literal
The compiler SHALL accept `НЕОПРЕДЕЛЕНО` / `UNDEFINED` as a literal of
kind `Undefined` rendered as SQL `NULL`. `<выражение> = НЕОПРЕДЕЛЕНО`
SHALL compare the value's type with the undefined type, so a composite
field holding the undefined value matches and no other value does;
comparing an expression whose kind cannot hold the undefined value SHALL
be a constant false predicate (`<>`: true), as the platform returns no
rows rather than an error.
In `ВЫБОР`, `ЕСТЬNULL`, and `ОБЪЕДИНИТЬ` the literal SHALL be
compatible with every kind like `NULL`; a column made only of the
literal SHALL report kind `Undefined`.

#### Scenario: Composite filter
- **WHEN** `ГДЕ Т.Объект = НЕОПРЕДЕЛЕНО` is compiled
- **THEN** generated SQL compares the encoded type of the field with
  `0x0100000000`

#### Scenario: Fixed field
- **WHEN** `ГДЕ Т.Цена = НЕОПРЕДЕЛЕНО` is compiled
- **THEN** generated SQL contains a false predicate and no diagnostic

### Requirement: Order by computed expressions
`УПОРЯДОЧИТЬ ПО` / `ORDER BY` SHALL accept any supported expression as an
ordering key, alongside the field path and projection alias forms. A
branch that orders by physical expressions SHALL render the compiled
expression in `ORDER BY`, keeping the written order of the keys and the
`ВОЗР`/`УБЫВ` direction. A branch whose ordering must name projected
columns — a joined branch, a grouped branch, or a union — SHALL refuse an
expression key with the diagnostic it already reports for a field that is
not projected. A statement with `ИТОГИ` SHALL project an expression key
as a hidden column so the totals wrapper can order by it. Ordering by a
type value SHALL follow the encoding, which puts the undefined type
first, then the primitive types, then references by table number, as the
platform orders them.

#### Scenario: Conditional ordering
- **WHEN** `… УПОРЯДОЧИТЬ ПО ВЫБОР КОГДА Цена > 10 ТОГДА 0 ИНАЧЕ 1 КОНЕЦ, Имя`
  is compiled for a single-source branch
- **THEN** generated SQL orders by that `CASE` and then by the name

#### Scenario: Ordering by a type
- **WHEN** `… УПОРЯДОЧИТЬ ПО ТИПЗНАЧЕНИЯ(Т.Объект), Имя` is executed over
  a composite attribute
- **THEN** the rows come out grouped by type in the platform's order

#### Scenario: Expression key in a joined branch
- **WHEN** an expression key is used in a branch with a join
- **THEN** compilation fails with `UnsupportedFeature` and the message
  that the ordering key must occur in the projection

### Requirement: Test compound fields for NULL
`ЕСТЬ [НЕ] NULL` / `IS [NOT] NULL` SHALL accept a compound field — a
composite attribute or a runtime-typed reference — and SHALL render the
test on one representative physical member: the `_TYPE` discriminator
when the field has one, otherwise the `RRRef` value member. A compound
field with neither member SHALL keep reporting that it can be projected
but not used in expressions.

#### Scenario: Composite attribute of a present row
- **WHEN** `ВЫБОР КОГДА Т.Объект ЕСТЬ NULL ТОГДА 1 ИНАЧЕ 0 КОНЕЦ` is
  executed over rows of the catalog itself
- **THEN** every row answers `0`, because the discriminator is always
  written

#### Scenario: Composite attribute of a missing join row
- **WHEN** the same test is applied to the composite attribute of a
  `ЛЕВОЕ СОЕДИНЕНИЕ` whose row is absent
- **THEN** the answer is `1`, and `ЕСТЬ НЕ NULL` answers `0`

### Requirement: Compile range predicates
The compiler SHALL accept `<выражение> [НЕ] МЕЖДУ <нижняя> И <верхняя>` /
`<expression> [NOT] BETWEEN <lower> AND <upper>` wherever comparisons are
accepted, with any supported scalar expressions as the value and the
bounds, and SHALL render `BETWEEN` / `NOT BETWEEN`. The bounds SHALL be
inclusive, reversed bounds SHALL select no rows, and a `NULL` value SHALL
not match, as on the platform.

#### Scenario: Numeric range
- **WHEN** `ГДЕ Т.Цена МЕЖДУ 8 И 22` is compiled
- **THEN** generated SQL is `(… BETWEEN 8 AND 22)` and the rows with the
  bound values are selected

#### Scenario: Negated range over expressions
- **WHEN** `ГДЕ Т.Цена НЕ МЕЖДУ Т.Цена - 1 И Т.Цена + 1` is compiled
- **THEN** generated SQL is `(NOT (… BETWEEN … AND …))`

### Requirement: Compile hierarchy membership
The compiler SHALL accept `<поле> [НЕ] В ИЕРАРХИИ (<список> | <запрос>)` /
`<field> [NOT] IN HIERARCHY (…)` wherever `В` is accepted. The predicate
SHALL be true when the value equals one of the seeds or descends from one
through the catalog's `_ParentIDRRef` chain, and `НЕ` SHALL negate it.
The descent SHALL be rendered as one recursive CTE per predicate, defined
at statement level — `WITH RECURSIVE` on PostgreSQL, `WITH` on SQL Server
— and tested with `EXISTS`, so a `NULL` seed cannot swallow the result. A
target catalog without a live parent column SHALL degenerate to plain
membership. The tested value SHALL be a field referencing exactly one
catalog, otherwise an `UnsupportedFeature` diagnostic; seeds SHALL be a
nested query or constants, and a field among them SHALL be an
`UnsupportedFeature` diagnostic. A statement that defines a temporary
table SHALL refuse the predicate.

#### Scenario: Folder subtree
- **WHEN** `ГДЕ Т.Ссылка В ИЕРАРХИИ (ВЫБРАТЬ Г.Ссылка ИЗ Справочник.Товары КАК Г ГДЕ Г.Наименование = "Мебель")`
  is executed over a catalog whose `Мебель` folder holds `Кухня`
- **THEN** the rows are `Мебель`, `Кухня`, and every item beneath them

#### Scenario: Item seed
- **WHEN** the seed is an item rather than a folder
- **THEN** only that item is selected

#### Scenario: Empty reference
- **WHEN** the seed is the empty reference of the catalog
- **THEN** every row of the catalog is selected, because every chain of
  parents ends at the empty reference

#### Scenario: Catalog without a hierarchy
- **WHEN** the target catalog has no parent column
- **THEN** the predicate is plain membership in the seeds

### Requirement: Compile the scalar string and arithmetic functions
The compiler SHALL accept `ПОДСТРОКА(x, n, m)`, `ДлинаСтроки(x)`,
`СокрЛП(x)`, `СокрЛ(x)`, `СокрП(x)`, `ВРег(x)`, `НРег(x)`, `Лев(x, n)`,
`Прав(x, n)`, `СтрНайти(x, s)`, `СтрЗаменить(x, s, r)`, `Окр(x[, n])`,
`Цел(x)`, `Sqrt`, `Exp`, `Log`, `Log10`, `Pow`, `Cos`, `Sin`, `Tan`,
`ACos`, `ASin`, and `ATan`, with their English spellings, and SHALL
render each per dialect. String functions SHALL return a string and the
others a number. An argument of the wrong kind SHALL be a `Syntax`
diagnostic, `NULL` and unclassified values passing. The arity SHALL be
checked at parse time. PostgreSQL renderings SHALL stay within the targeted
minimum server,
and a character column of the 1C extension types SHALL be cast to `text`
before the call. `Log` SHALL be the natural logarithm, `Log10` the
decimal one, `Окр` SHALL round half away from zero and accept a negative
scale, and `Цел` SHALL truncate toward zero, as measured on the platform.

#### Scenario: String functions
- **WHEN** `ПОДСТРОКА(Т.Наименование, 2, 3)`, `Лев(Т.Наименование, 3)`,
  and `СтрНайти(Т.Наименование, "ан")` are compiled
- **THEN** PostgreSQL renders `substring(… from 2 for 3)`,
  `substring(… from 1 for 3)`, and `position('ан' in …)`, and the results
  match the platform row for row

#### Scenario: Trailing spaces
- **WHEN** `ДлинаСтроки("  а  ")` is compiled
- **THEN** the answer is 5 on both providers, so SQL Server cannot use
  `LEN` alone

#### Scenario: Wrong argument kind
- **WHEN** `ВРег(Т.Цена)` is compiled
- **THEN** compilation fails with a `Syntax` diagnostic naming the
  expected kind

### Requirement: Group register virtual tables by the dimensions in use
`Остатки` and `Обороты` SHALL answer one row per combination of the
dimensions the statement resolves against the source, summing their
resources over every other dimension, as the platform does. A statement
that resolves no dimension SHALL get exactly one row. A dimension named
only inside the virtual table's own condition SHALL not add a grouping
level. Every virtual table SHALL also be accepted without its argument
list, which is how the platform writes it when no argument is given.

#### Scenario: One dimension of two
- **WHEN** `ВЫБРАТЬ О.Товар, О.КоличествоОборот ИЗ РегистрНакопления.Продажи.Обороты КАК О`
  is executed over a register with dimensions `Товар` and `Клиент`
- **THEN** there is one row per товар, its turnover summed over клиенты

#### Scenario: No dimension
- **WHEN** only a resource is selected
- **THEN** there is exactly one row holding the turnover of the register

#### Scenario: Condition on an unread dimension
- **WHEN** the virtual table's condition filters by `Клиент` and the
  statement selects only the resource
- **THEN** there is one row, filtered but not grouped

### Requirement: Compile turnovers by period
`РегистрНакопления.X.Обороты(Начало, Конец, Периодичность, Условие)` SHALL
accept a calendar periodicity from `Секунда` to `Год`, written as a bare
period name, and SHALL group the turnovers by the beginning of the period
each record falls into, exposing it as the `Период` field. The grouping
SHALL apply even when the statement never reads `Период`, as the platform
answers one row per period. `Регистратор` and `Запись` SHALL be an
`UnsupportedFeature` diagnostic.

#### Scenario: Monthly turnovers
- **WHEN** `ВЫБРАТЬ О.Период, О.Товар, О.КоличествоОборот ИЗ РегистрНакопления.Продажи.Обороты(, , Месяц, ) КАК О`
  is executed
- **THEN** there is one row per month and товар, the period being the
  first day of the month

#### Scenario: Period never read
- **WHEN** only the resource is selected from a monthly turnover table
- **THEN** there is still one row per month

#### Scenario: Recorder periodicity
- **WHEN** the periodicity is `Регистратор`
- **THEN** compilation fails with an `UnsupportedFeature` diagnostic

### Requirement: Compile the balance-and-turnovers table
`РегистрНакопления.X.ОстаткиИОбороты(Начало, Конец, Периодичность,
МетодДополненияПериодов, Условие)` SHALL compile for a balance register
and SHALL expose, per resource, `НачальныйОстаток`, `Приход`, `Расход`,
`Оборот` and `КонечныйОстаток` beside the register's dimensions. The
opening balance SHALL be the signed movement before `Начало`, the
receipts and expenses the movements of `[Начало, Конец)` split by record
kind, the turnover their difference, and the closing balance the sum of
the opening balance and the turnover. The five columns SHALL be summed
over the dimensions the statement never reads, like every register
table. A turnover-only register SHALL be refused. The periodicity and
the period completion method SHALL be `UnsupportedFeature` diagnostics.

#### Scenario: Whole register
- **WHEN** `ВЫБРАТЬ О.Товар, О.КоличествоПриход, О.КоличествоКонечныйОстаток ИЗ РегистрНакопления.Продажи.ОстаткиИОбороты КАК О`
  is executed
- **THEN** there is one row per товар with its receipts and closing
  balance

#### Scenario: Interval
- **WHEN** the table is read over `[2024-02-01, 2024-05-01)`
- **THEN** the opening balance holds the movements before February and
  the closing balance adds the turnover of the interval

#### Scenario: Periodicity
- **WHEN** a periodicity is given
- **THEN** compilation fails with an `UnsupportedFeature` diagnostic

### Requirement: Compile turnovers by recorder
`РегистрНакопления.X.Обороты` SHALL accept `Регистратор` and `Запись` as
its periodicity. `Регистратор` SHALL group the turnovers by the period
and the recorder of the records and SHALL expose the `Период` and
`Регистратор` fields; `Запись` SHALL additionally group by and expose
`НомерСтроки`, so each register record answers its own row. Neither
grouping SHALL be dropped when the statement does not read the columns.
`НомерСтроки` SHALL stay unavailable under `Регистратор`, as on the
platform.

#### Scenario: Turnovers of each document
- **WHEN** `ВЫБРАТЬ О.Период, О.Регистратор, О.КоличествоОборот ИЗ РегистрНакопления.Продажи.Обороты(, , Регистратор, ) КАК О`
  is executed
- **THEN** there is one row per document and dimension combination in use

#### Scenario: Each record
- **WHEN** the periodicity is `Запись`
- **THEN** `НомерСтроки` is available and each register record answers
  its own row

### Requirement: Walk reference paths of any depth
A field path SHALL dereference any number of single-target references,
joining each hop's target to the alias the previous hop produced and
reading the last segment from the table the walk ended on. Identical hops
of one branch SHALL share one join. The path SHALL work wherever a
one-hop path does: projections, `ГДЕ`, `СГРУППИРОВАТЬ ПО` and
`УПОРЯДОЧИТЬ ПО`. A path that continues through a composite reference
SHALL be an `UnsupportedFeature` diagnostic naming that field, because
such a reference selects its value by type and has no single table to
continue from.

#### Scenario: Two hops
- **WHEN** `ВЫБРАТЬ Т.Поставщик.Родитель.Наименование ИЗ Справочник.Товары КАК Т`
  is executed
- **THEN** the answer is the name of the supplier's folder, and `NULL`
  where either reference is empty

#### Scenario: Shared prefix
- **WHEN** a query reads `Т.Поставщик.Родитель.Наименование` and
  `Т.Поставщик.Родитель.Код`
- **THEN** the generated SQL joins the supplier once and its folder once

#### Scenario: Composite in the middle
- **WHEN** the path continues through a composite reference
- **THEN** compilation fails with an `UnsupportedFeature` diagnostic

### Requirement: Dereference standard fields through a composite reference
A dereference through a reference that admits several tables SHALL
resolve standard fields as well as attributes. SchemaStorage names no
target for such a reference, so the candidates are scanned from the
snapshot; the scan SHALL treat every accepted spelling of a standard
field as present in every reference object and leave the candidate limit
and its `ВЫРАЗИТЬ` advice unchanged.

#### Scenario: Recorder date
- **WHEN** `ВЫБРАТЬ О.Регистратор.Дата ИЗ РегистрНакопления.Продажи КАК О`
  is executed
- **THEN** the answer is the date of the document that wrote each record

#### Scenario: Parent of a composite attribute
- **WHEN** `ВЫБРАТЬ Т.Объект.Родитель ИЗ Справочник.Товары КАК Т` is
  executed over an attribute holding references to two catalogs
- **THEN** the rows whose value is a catalog with a parent answer it, and
  the others answer `NULL`

### Requirement: Keep a recorded corpus of real queries
The repository SHALL carry the query texts of a real 1C configuration
together with the result the compiler produces for each, and a metadata
fixture of that configuration pruned to the objects those queries reach.
A test SHALL recompile every query against the fixture and compare the
result with the recorded one, failing on any difference, and SHALL assert
the number of queries that compile.

Only text the lexer and parser accept as a query SHALL be recorded, so
that interface captions beginning with a query keyword stay out of the
corpus. Each recorded entry SHALL carry the values its parameters take, so
a query is compiled against a value of the type its use requires rather
than against an untyped `NULL`. An entry whose metadata the pruned fixture
does not carry SHALL be recorded as such, and the asserted count SHALL
cover the applicable entries only.

#### Scenario: Unchanged compiler
- **WHEN** the corpus test runs against an unchanged compiler
- **THEN** every query produces its recorded SQL or diagnostic

#### Scenario: Improvement
- **WHEN** a change makes a previously refused query compile
- **THEN** the test fails with the difference, and the recorded result and
  the count are updated in the same commit

#### Scenario: Recording a caption
- **WHEN** the corpus is re-recorded from a configuration whose texts
  include an interface caption such as `Выбрать пользователя`
- **THEN** the caption is not recorded, because no query can be parsed
  from it

#### Scenario: Query with a typed parameter
- **WHEN** a recorded query slices a register with `&ДатаОкончания`
- **THEN** the harness binds that parameter to a date and the query
  compiles, instead of being refused for an untyped parameter

#### Scenario: Entry beyond the fixture
- **WHEN** a recorded query names a metadata object the pruned fixture
  does not carry
- **THEN** the entry is reported as inapplicable rather than as a
  compiler gap, and it is excluded from the asserted count

### Requirement: Resolve the computed standard fields
`ЭтоГруппа` / `IsFolder` SHALL resolve to the negation of the stored
`Folder` column, and `Предопределенный` SHALL resolve to the
`PredefinedID` column differing from the empty reference, because the
platform computes both rather than storing them. They SHALL answer in a
projection, a predicate, a grouping key, an ordering key and through a
reference, SHALL carry the boolean column kind, and SHALL be spelled as a
bit on SQL Server.

#### Scenario: Folders of a catalog
- **WHEN** `ВЫБРАТЬ Т.ЭтоГруппа ИЗ Справочник.Товары КАК Т ГДЕ Т.ЭтоГруппа`
  is executed over a hierarchical catalog
- **THEN** only the folders answer, with the value true, exactly as on
  the platform

#### Scenario: Predefined items
- **WHEN** `ВЫБРАТЬ Т.Предопределенный ИЗ Справочник.Товары КАК Т` is
  executed
- **THEN** only the predefined items answer true

#### Scenario: Through a reference
- **WHEN** the field is read as `Т.Поставщик.ЭтоГруппа`
- **THEN** it answers from the joined table, and `NULL` where the
  reference is empty

### Requirement: Resolve the predefined-data name
`ИмяПредопределенныхДанных` / `PredefinedDataName` SHALL resolve to the
symbolic name of the predefined item a row is, taken from the predefined
values of the owning object, and to an empty string for a row that is not
predefined, because the platform derives the name from the stored
`PredefinedID` rather than storing it. It SHALL answer in a projection, a
predicate, a grouping key and an ordering key, SHALL carry the string
column kind, and SHALL be refused on an object without a `PredefinedID`
column. Through a reference it SHALL answer from the joined table and
stay `NULL` where the reference matched no row.

#### Scenario: Predefined items of a catalog
- **WHEN** `ВЫБРАТЬ Т.ИмяПредопределенныхДанных ИЗ Справочник.Товары КАК Т`
  is executed
- **THEN** each predefined item answers its declared name and every other
  row answers an empty string, exactly as on the platform

#### Scenario: Filtering by the name
- **WHEN** the field is compared with a declared name in `ГДЕ`
- **THEN** only the item declared under that name answers

#### Scenario: Through a reference
- **WHEN** the field is read as `Т.Поставщик.ИмяПредопределенныхДанных`
- **THEN** it answers from the joined table, and `NULL` where the
  reference is empty

#### Scenario: Object without predefined data
- **WHEN** the field is read from a document
- **THEN** the compiler reports an unknown field, as the platform does

### Requirement: Rendering a deferred presentation
A deferred presentation column that the database answers as `NULL` SHALL
stay `NULL` in the console output, because a row carrying no reference has
no presentation. A reference that no object answers SHALL keep its
unresolved marker.

#### Scenario: A row without a reference
- **WHEN** a deferred presentation column is `NULL`
- **THEN** the console prints it as `NULL`

#### Scenario: A reference no object answers
- **WHEN** the lookup finds no object for a reference
- **THEN** the console prints the unresolved marker

### Requirement: Read a single constant as its own table
A source written as `Константа.<Имя>` / `Constant.<Name>` SHALL expose the
stored value under the field name `Значение` / `Value`, not under the name
of the constant, because that is the name the platform answers to. The
field SHALL behave like any other field of the source: it SHALL project,
filter, group, order, dereference when it holds a reference, and carry the
column kind of its stored type. The separator predicates of the constant's
table SHALL apply as they do to any other source.

#### Scenario: Value of a constant
- **WHEN** `ВЫБРАТЬ К.Значение ИЗ Константа.ОсновнойТовар КАК К` is
  compiled
- **THEN** the stored value column of the constant's table is projected,
  and the result matches the platform

#### Scenario: Constant addressed by its own name
- **WHEN** `ВЫБРАТЬ К.ОсновнойТовар ИЗ Константа.ОсновнойТовар КАК К` is
  compiled
- **THEN** compilation fails with an unknown-field diagnostic, as the
  platform reports "Поле не найдено"

#### Scenario: Dereferenced constant value
- **WHEN** the constant stores a reference and
  `ВЫБРАТЬ К.Значение.Наименование ИЗ Константа.ОсновнойТовар КАК К` is
  compiled
- **THEN** the reference is joined and its description is projected

### Requirement: Qualify a source by its full metadata name
A field MAY be qualified by the full metadata name of a source that was
written without an alias, as in `ВЫБРАТЬ Справочник.Товары.Наименование ИЗ
Справочник.Товары`. When the source carries an alias, the full name SHALL
NOT resolve, because the alias replaces the name — the platform reports
"Поле не найдено" for that shape.

#### Scenario: Unaliased source qualified by its name
- **WHEN** `ВЫБРАТЬ Справочник.Товары.Наименование ИЗ Справочник.Товары` is
  compiled
- **THEN** the field resolves against that source and the result matches
  the platform

#### Scenario: Aliased source addressed by its name
- **WHEN** `ВЫБРАТЬ Т.Наименование ИЗ Справочник.Товары КАК Т ГДЕ
  Справочник.Товары.Цена > 0` is compiled
- **THEN** compilation fails, because the alias replaces the name

#### Scenario: Full name inside a subquery condition
- **WHEN** a correlated subquery selects from `Задача.ЗадачаИсполнителя`
  without an alias and its condition names
  `Задача.ЗадачаИсполнителя.Выполнена`
- **THEN** the field resolves against that source

### Requirement: Project a tabular section
A projection MAY name a tabular section of a source — `Д.Товары`,
`Д.Товары.(Поле, …)`, or `Д.Товары.*` — which the platform answers as a
nested result inside that column. `Д.Товары` SHALL select the owner
reference, the line number and every attribute of the section; the
parenthesized form SHALL select exactly the named columns; `Д.Товары.*`
SHALL select the same columns as `Д.Товары`. A section used anywhere but a
projection SHALL keep failing with `UnsupportedFeature`.

#### Scenario: Section projected whole
- **WHEN** `ВЫБРАТЬ Д.Номер, Д.Товары ИЗ Документ.Продажа КАК Д` is
  compiled
- **THEN** the query compiles and its nested result carries the rows of the
  section, matching the platform column for column

#### Scenario: Section in a predicate
- **WHEN** a `ГДЕ` names a tabular section
- **THEN** compilation fails, because a section is a table and not a value

### Requirement: Carry a unique identifier in a composite value
A value whose branches differ in type and include a unique identifier or
raw bytes SHALL compile, spreading the identifier over a member `_U` and
the bytes over a member `_B` of the composite value. The type tag of such
a branch SHALL be the `Null` tag, which is what the platform answers for
`ТИПЗНАЧЕНИЯ` of such a value. Branches of other types SHALL write the
zero of those members, as they do for every other member.

The layout is an extension of what 1C stores: the platform keeps a
composite value over `_TYPE/_L/_N/_T/_S/_RTRef/_RRRef` and has no binary
member, because such a value never reaches a table.

#### Scenario: Identifier beside a reference
- **WHEN** `ВЫБОР КОГДА Т.Цена > 5 ТОГДА УНИКАЛЬНЫЙИДЕНТИФИКАТОР(Т.Ссылка)
  ИНАЧЕ Т.Клиент КОНЕЦ` is compiled
- **THEN** the value carries the identifier in `_U`, the reference in the
  payload, and the row's type tag says which branch answered

#### Scenario: Stored binary field beside a reference
- **WHEN** a branch reads a field stored as raw bytes and another reads a
  reference
- **THEN** the bytes are carried in `_B` and the value compiles

#### Scenario: Type of such a value
- **WHEN** `ТИПЗНАЧЕНИЯ` is applied to that value
- **THEN** the identifier rows answer the `Null` type, as the platform
  answers

### Requirement: Test a tabular-section column in a predicate
A comparison MAY name a column of a tabular section of one of the
statement's sources as `<источник>.<Состав>.<Поле>`. It SHALL compile into
an `EXISTS` over the section correlated with the owner row, because that
is what the platform answers: a row of the owner appears once when any of
its section rows satisfies the comparison, never once per matching row.

The path SHALL remain refused outside a comparison — in a projection, a
grouping key or an ordering key — where existence is not its meaning.

#### Scenario: Owner with several matching rows
- **WHEN** `ВЫБРАТЬ Д.Номер ИЗ Документ.Продажа КАК Д ГДЕ
  Д.Товары.Количество > 2` is compiled over a document whose section has
  two rows above two
- **THEN** that document answers once, as the platform answers, and
  `КОЛИЧЕСТВО(*)` counts it once

#### Scenario: Correlated with an outer source
- **WHEN** a subquery selects from a task and compares
  `Задача.ЗадачаИсполнителя.Предметы.Предмет` with a column of the outer
  query
- **THEN** the `EXISTS` carries both correlations

#### Scenario: Path in a projection
- **WHEN** a projection names `Д.Товары.Количество`
- **THEN** compilation fails, because a section column is not a value of
  the owner row

### Requirement: Present a ВЫБОР whose branches differ in type
`ПРЕДСТАВЛЕНИЕ` of a `ВЫБОР` SHALL present each branch on its own and
answer one string column: a string branch answers itself, a reference
branch answers the presentation the application's plan builds, and a
`NULL` branch answers `NULL`. The branches SHALL NOT be required to share
one kind, because the platform answers such a `ВЫБОР` branch by branch.

A branch that can only be answered by the deferred protocol — a reference
expression that is not a field — SHALL fail with `UnsupportedFeature`,
since a column is either deferred as a whole or built from a plan.

#### Scenario: String beside a reference
- **WHEN** `ПРЕДСТАВЛЕНИЕ(ВЫБОР КОГДА Т.Цена > 5 ТОГДА "нет" ИНАЧЕ
  Т.Клиент КОНЕЦ)` is compiled
- **THEN** the string rows answer the string and the reference rows answer
  the presentation of the reference, as the platform answers

#### Scenario: Composite field as the subject and a branch
- **WHEN** the `ВЫБОР` chooses by a composite reference field and falls
  back to that field
- **THEN** the presentation compiles, with the composite branch presented
  through its targets

### Requirement: Present an aggregate
`ПРЕДСТАВЛЕНИЕ` MAY take an aggregate as its argument. Such a projection
SHALL count as an aggregated projection, so the branch aggregates like any
other, whether or not it groups. A non-reference aggregate SHALL answer its
own value as text; a reference aggregate SHALL answer through the
presentation protocol, as a reference expression does.

#### Scenario: Presented count
- **WHEN** `ВЫБРАТЬ ПРЕДСТАВЛЕНИЕ(КОЛИЧЕСТВО(*)) ИЗ Справочник.Товары` is
  compiled
- **THEN** the branch aggregates and the column answers the count as text,
  as the platform answers

#### Scenario: Presented reference aggregate
- **WHEN** `ВЫБРАТЬ ПРЕДСТАВЛЕНИЕ(МАКСИМУМ(Т.Клиент)) ИЗ Справочник.Товары
  КАК Т` is compiled
- **THEN** the column carries the greatest reference for the presentation
  protocol to resolve, which answers what the platform answers

#### Scenario: Presented aggregate of a group
- **WHEN** the same projection appears beside `СГРУППИРОВАТЬ ПО`
- **THEN** every group answers its own aggregate

### Requirement: Test a composite reference in В ИЕРАРХИИ
`В ИЕРАРХИИ` MAY test a field whose value is a composite reference. The
identifier member SHALL be compared with the seeds and their descendants,
and the type member SHALL be required to equal the type of the catalog the
seeds belong to, because a value of another type is under no seed. When
the seeds' catalog has no parent column the predicate SHALL degenerate to
membership, exactly as it does for a single-target field.

#### Scenario: Composite value under a group
- **WHEN** a catalog attribute holds either a product or a client and the
  predicate names a product group as its seed
- **THEN** the rows holding a product under that group answer, and the
  rows holding a client do not

#### Scenario: Branch of unbound parameters beside a reference
- **WHEN** an alternative mixes a nested `ВЫБОР` whose branches are all
  `NULL` with a branch carrying a reference
- **THEN** the untyped branches are rendered as `NULL` of the reference
  type, so the server accepts the alternative

#### Scenario: Seeds of a catalog without hierarchy
- **WHEN** the seeds are clients, whose catalog has no parent column
- **THEN** the predicate answers the rows whose value is that client,
  as the platform answers

### Requirement: Compare a composite column of a tabular section
The `EXISTS` of a tabular-section predicate MAY compare a column stored as
a reference pair. Such a column SHALL be compared by its `RTRef ‖ RRRef`
payload, and the other side SHALL be widened to a payload as it is in any
other comparison of a composite reference. A column that is composite in
another way SHALL keep its diagnostic.

#### Scenario: Section column holding any reference
- **WHEN** a predicate compares `Задача.ЗадачаИсполнителя.Предметы.Предмет`
  with a catalog reference
- **THEN** the `EXISTS` compares the payload of the section column with
  the widened reference, and the owner answers once

#### Scenario: Section column that is not a reference pair
- **WHEN** the compared column spreads over members that are not a
  reference pair
- **THEN** compilation fails, naming the column

### Requirement: Compile system enumeration values
`ЗНАЧЕНИЕ`/`VALUE` SHALL accept a two-segment path naming a system
enumeration and one of its values, bilingually, and SHALL compile it to
the number the platform stores for that value: `ВидДвиженияНакопления`
(`AccumulationRecordType`) with `Приход`/`Receipt` 0 and
`Расход`/`Expense` 1; `ВидДвиженияБухгалтерии` (`AccountingRecordType`)
with `Дебет`/`Debit` 0 and `Кредит`/`Credit` 1; `ВидСчета`
(`AccountType`) with `Активный`/`Active` 0, `Пассивный`/`Passive` 1 and
`АктивноПассивный`/`ActivePassive` 2. The expression SHALL be accepted
wherever a numeric literal is, including the conditions of virtual
tables, and an unknown enumeration or value SHALL be a `Syntax`
diagnostic naming it.

#### Scenario: Movement kind in a predicate
- **WHEN** `ГДЕ Т.ВидДвижения = ЗНАЧЕНИЕ(ВидДвиженияНакопления.Расход)` is compiled
- **THEN** the SQL compares the `_RecordKind` column with `1`

#### Scenario: Account kind
- **WHEN** `ВЫБОР КОГДА О.Счет.Вид = ЗНАЧЕНИЕ(ВидСчета.АктивноПассивный) ТОГДА …` is compiled
- **THEN** the dereferenced `_Kind` column of the chart of accounts is
  compared with `2`

#### Scenario: Unknown value
- **WHEN** `ЗНАЧЕНИЕ(ВидСчета.Дебет)` is compiled
- **THEN** compilation fails with a `Syntax` diagnostic that names the value

### Requirement: Expose chart-of-accounts standard fields
A chart of accounts SHALL expose `Вид`/`Kind`, `Забалансовый`/`OffBalance`
and `Порядок`/`Order` as standard fields, also through a dereference.

#### Scenario: Off-balance accounts
- **WHEN** `ГДЕ Счета.Забалансовый` is compiled over `ПланСчетов.Управленческий`
- **THEN** the SQL reads the `_OffBalance` column

### Requirement: Accept a zero row count
`ПЕРВЫЕ 0`/`TOP 0` SHALL be accepted like any other count and SHALL
render as `LIMIT 0` on PostgreSQL and `TOP (0)` on SQL Server, so the
statement answers its columns and no rows, as on the platform.

#### Scenario: Empty temporary table
- **WHEN** `ВЫБРАТЬ ПЕРВЫЕ 0 Т.Ссылка КАК Ссылка ПОМЕСТИТЬ ВТ ИЗ Справочник.Номенклатура КАК Т` is compiled
- **THEN** the PostgreSQL text ends the temporary table's statement with
  `LIMIT 0` and no diagnostic is reported

### Requirement: Expose receipts and expenses on turnovers
`РегистрНакопления.<Имя>.Обороты` of a balance register SHALL expose,
per resource, `<Ресурс>Приход` (`<Resource>Receipt`) — the sum of the
resource over the receipt records of the interval — and `<Ресурс>Расход`
(`<Resource>Expense`) — the sum over the expense records — beside
`<Ресурс>Оборот`, and SHALL sum them over the dimensions the statement
never reads like every resource column. A turnover-only register SHALL
keep exposing the turnover only.

#### Scenario: Receipts of an order
- **WHEN** `ВЫБРАТЬ О.КоличествоПриход, О.КоличествоРасход ИЗ РегистрНакопления.Заказы.Обороты(, , , Заказ = &Заказ) КАК О` is compiled
- **THEN** the SQL sums the resource over records with `_RecordKind = 0`
  for the receipt and `_RecordKind = 1` for the expense

#### Scenario: Turnover-only register
- **WHEN** `О.КоличествоПриход` is read from a turnover-only register
- **THEN** compilation fails with an `UnknownField` diagnostic

### Requirement: Accept the period and auto periodicities
The periodicity argument of `Обороты` and `ОстаткиИОбороты` SHALL accept
`Период`/`Period` and `Авто`/`Auto` and SHALL compile either like an
omitted periodicity: the table answers one row per combination of the
dimensions in use over the whole interval and exposes no `Период`
column. (`Период` is the platform's documented default; `Авто` splits by
the period fields a query reads, which the compiler refuses, so without
them it is the default too.)

#### Scenario: Turnovers for the whole period
- **WHEN** `РегистрНакопления.Продажи.Обороты(&Н, &К, Период, )` is read by товар
- **THEN** the SQL groups by товар only and applies the interval bounds

#### Scenario: Auto without period fields
- **WHEN** `РегистрНакопления.Продажи.ОстаткиИОбороты(&Н, &К, Авто, , )` is read
- **THEN** it compiles like the table without a periodicity

### Requirement: Expose the change-registration fields
The change-registration table of an object SHALL expose `Узел`/`Node` —
the exchange-plan node the change is registered for, with the reference
behaviour of any reference field — and `НомерСообщения`/`MessageNo` as
standard fields beside the registered object's key.

#### Scenario: Changes of one node
- **WHEN** `ВЫБРАТЬ И.Ссылка ИЗ Справочник.Номенклатура.Изменения КАК И ГДЕ И.Узел = &Узел И И.НомерСообщения ЕСТЬ NULL` is compiled
- **THEN** the SQL compares the node columns of the change table with
  the parameter and tests `_MessageNo` for NULL

### Requirement: Report a parameter used as a source
A parameter written where a source is expected (`ИЗ &Таблица`, or after
a join keyword) SHALL be reported as an `UnsupportedFeature` diagnostic
positioned at the parameter and stating that a table passed as a
parameter is not supported.

#### Scenario: Value table parameter
- **WHEN** `ВЫБРАТЬ Т.Номенклатура ИЗ &ТаблицаТоваров КАК Т` is compiled
- **THEN** compilation fails with `UnsupportedFeature` at `&ТаблицаТоваров`

### Requirement: Split by the fields the statement reads under Auto
Under the `Авто`/`Auto` periodicity, `Обороты` and `ОстаткиИОбороты`
SHALL expose `Период` (the record period), `ПериодСекунда`, `ПериодМинута`,
`ПериодЧас`, `ПериодДень`, `ПериодНеделя`, `ПериодДекада`, `ПериодМесяц`,
`ПериодКвартал`, `ПериодПолугодие`, `ПериодГод` (`SecondPeriod` …
`YearPeriod`: the beginning of that period of the record), `Регистратор`
and `НомерСтроки`, and SHALL treat them as dimensions of the relation:
the ones the statement reads split the rows, the others are summed away
like an unread dimension. `ОстаткиИОбороты` SHALL refuse its balance
columns with an `UnsupportedFeature` diagnostic when the statement reads
one of these split fields, and SHALL answer them otherwise; a period
completion method SHALL be accepted with `Авто`.

#### Scenario: Turnovers by month and recorder
- **WHEN** `ВЫБРАТЬ О.ПериодМесяц, О.Регистратор, О.СуммаОборот ИЗ РегистрНакопления.Продажи.Обороты(&Н, &К, Авто, ) КАК О` is compiled
- **THEN** the SQL groups by the beginning of the month of the record
  period and by the recorder columns

#### Scenario: Auto without split fields
- **WHEN** the statement reads dimensions and resources only
- **THEN** the SQL is the same as without a periodicity

#### Scenario: Balance with a split field
- **WHEN** `О.Регистратор` and `О.СуммаКонечныйОстаток` are read from `ОстаткиИОбороты(&Н, &К, Авто, ДвиженияИГраницыПериода, )`
- **THEN** compilation fails with an `UnsupportedFeature` diagnostic

### Requirement: Compile tuple membership tests
`(<expression>, <expression>, …) [НЕ] В (<query>)` SHALL compile when
the subquery projects one column per tuple item of a compatible kind:
the test SHALL render as `EXISTS` over the subquery with an equality per
column, `NOT EXISTS` when negated, on both dialects, and SHALL be
accepted wherever a predicate is, including the condition of a virtual
table. A tuple anywhere else, a column count that differs from the
tuple, an incompatible column, or a reference of several types on either
side SHALL be an `UnsupportedFeature` diagnostic.

#### Scenario: Pair in a slice condition
- **WHEN** `РегистрСведений.Цены.СрезПоследних(&Дата, (Номенклатура, Характеристика) В (ВЫБРАТЬ С.Номенклатура, С.Характеристика ИЗ Документ.Заказ.Товары КАК С ГДЕ С.Ссылка = &Заказ))` is compiled
- **THEN** the slice's condition holds `EXISTS (SELECT 1 FROM (…) AS "__in" WHERE "__in"."Номенклатура" = … AND "__in"."Характеристика" = …)`

#### Scenario: Column count mismatch
- **WHEN** a two-item tuple is tested against a one-column subquery
- **THEN** compilation fails with an `UnsupportedFeature` diagnostic

### Requirement: Expose accounting-register main-table fields
The main table `РегистрБухгалтерии.<Имя>` SHALL expose `СчетДт`/`СчетКт`
(`AccountDr`/`AccountCr`) for a register with correspondence and `Счет`
(`Account`) without it, every balance dimension and resource under its
own name, every non-balance dimension and resource as `<Имя>Дт`/`<Имя>Кт`
(`<Name>Dr`/`<Name>Cr`), the attributes, and the standard fields `Период`,
`Регистратор`, `НомерСтроки`, `Активность`. The names come from Config
purposes and the physical side suffix of the column, never guessed from
a column's data.

#### Scenario: Debit account
- **WHEN** `ВЫБРАТЬ Т.СчетДт, Т.Сумма, Т.СуммаВалДт ИЗ РегистрБухгалтерии.Управленческий КАК Т`
  is compiled
- **THEN** the SQL reads the debit account column, the balance resource
  column, and the debit column of the non-balance resource

#### Scenario: Non-balance name without a side
- **WHEN** a non-balance resource is read as `Т.СуммаВал`
- **THEN** compilation fails with an `UnknownField` diagnostic

### Requirement: Parse accounting virtual tables with the platform's arity
`РегистрБухгалтерии.<Имя>.Остатки`, `.Обороты`, `.ОстаткиИОбороты`,
`.ОборотыДтКт` and `.ДвиженияССубконто` SHALL accept at most 4, 8, 7, 8
and 5 arguments respectively, and SHALL be reported as an
`UnsupportedFeature` diagnostic naming the table until the stage that
compiles them lands; one argument more SHALL stay a `Syntax` diagnostic.

#### Scenario: Turnovers with an account condition
- **WHEN** `РегистрБухгалтерии.Управленческий.Обороты(&Н, &К, , Счет = &Счет, , , , )` is compiled
- **THEN** compilation fails with `UnsupportedFeature`, not with a
  syntax error about the argument count

### Requirement: Resolve predefined accounts
`ЗНАЧЕНИЕ`/`VALUE` SHALL accept a `ПланСчетов` object and SHALL resolve
the named predefined account through the chart's `_PredefinedID` column,
as it does for a catalog. Charts of characteristic types and of
calculation types SHALL stay refused until their resources are measured.

#### Scenario: Predefined account
- **WHEN** `ГДЕ О.Счет = ЗНАЧЕНИЕ(ПланСчетов.Управленческий.ПрочиеРасходы)` is compiled
- **THEN** the SQL selects `_IDRRef` of the chart's table by
  `_PredefinedID` equal to the stable identifier of `ПрочиеРасходы`

### Requirement: Compile accounting turnovers
`РегистрБухгалтерии.<Имя>.Обороты(Начало, Конец, Периодичность,
УсловиеСчета, Субконто, Условие, УсловиеКорСчета, КорСубконто)` of a
register with correspondence — or `Обороты(Начало, Конец, Периодичность,
УсловиеСчета, Условие, УсловиеКорСчета)` when the register keeps no
extra dimensions, the platform omitting the `Субконто` arguments then
(measured on the UNF configuration) — SHALL answer, per account (`Счет`), the
dimensions in use and the calendar period when a periodicity is given,
`<Ресурс>Оборот` (debit minus credit), `<Ресурс>ОборотДт` and
`<Ресурс>ОборотКт`, computed from the active records of `[Начало,
Конец)` folded into a debit row and a credit row each; a non-balance
dimension or resource SHALL be read from the side's own column under its
side-less name. Unread dimensions SHALL be summed away like every
register table. `УсловиеСчета` SHALL be a predicate on `Счет`, the
side's account; `Условие` SHALL be a predicate on the side's view of the
record. The extra-dimension list, the balanced-account arguments, the
`Авто` periodicity and a register without correspondence SHALL be
`UnsupportedFeature` diagnostics naming what is missing; `Регистратор`
and `Запись` SHALL split the rows like the accumulation table does.

#### Scenario: Turnovers by account and organization
- **WHEN** `ВЫБРАТЬ О.Счет, О.Организация, О.СуммаОборотДт ИЗ РегистрБухгалтерии.Управленческий.Обороты(&Н, &К, , Счет = &Счет) КАК О`
  is compiled
- **THEN** the SQL unions a debit branch and a credit branch of the main
  table, applies the account condition to each side's account, and sums
  the debit rows into `СуммаОборотДт`

#### Scenario: Non-balance resource
- **WHEN** `О.СуммаВалОборотКт` is read
- **THEN** the credit branch reads `_Fld<N>Ct` and the debit branch
  `_Fld<N>Dt` under one column, and the credit sum answers the column

#### Scenario: Condition fifth without extra dimensions
- **WHEN** `Обороты(&Н, &К, МЕСЯЦ, , СценарийПланирования = &С)` is compiled for the UNF register
- **THEN** the fifth argument is the condition and a seventh argument is
  a `Syntax` diagnostic

#### Scenario: Balanced account requested
- **WHEN** the balanced-account condition is given
- **THEN** compilation fails with an `UnsupportedFeature` diagnostic

### Requirement: Compile accounting balances
`РегистрБухгалтерии.<Имя>.Остатки(Период, УсловиеСчета, [Субконто],
Условие)` of a register with correspondence SHALL answer, per account
and dimensions in use, `<Ресурс>Остаток` — the debit rows minus the
credit rows of the active records before `Период` (all records when it
is omitted) — and `<Ресурс>ОстатокДт`/`<Ресурс>ОстатокКт` as the
positive part and the negated negative part of that balance at the grain
the statement reads, computed after unread dimensions are summed away,
and SHALL drop combinations whose every balance is zero. The
`Субконто` argument SHALL be absent for a register without extra
dimensions.

#### Scenario: Balance parts after pruning
- **WHEN** `ВЫБРАТЬ О.Счет, О.СуммаОстатокДт ИЗ РегистрБухгалтерии.Управленческий.Остатки(&Д) КАК О`
  is compiled
- **THEN** the organization is summed away first and the debit part is
  the positive part of the summed balance, not a sum of the parts

#### Scenario: Zero balance
- **WHEN** every resource balance of one combination is zero
- **THEN** the combination is absent from the result

### Requirement: Compile accounting balances and turnovers
`РегистрБухгалтерии.<Имя>.ОстаткиИОбороты(Начало, Конец, Периодичность,
МетодДополненияПериодов, УсловиеСчета, [Субконто], Условие)` SHALL
answer, per account and dimensions in use, `<Ресурс>НачальныйОстаток`
(the balance of the records before `Начало`), `<Ресурс>Оборот`,
`<Ресурс>ОборотДт`, `<Ресурс>ОборотКт` of `[Начало, Конец)`, and
`<Ресурс>КонечныйОстаток`, with the debit and credit parts of both
balances derived at the grain read. A calendar or record periodicity
SHALL split the rows and refuse the balance columns; `Авто` SHALL expose
the split fields as dimensions and refuse the balance columns only when
one of them is read; the completion method follows the accumulation
table's rules.

#### Scenario: Balances by account
- **WHEN** `ВЫБРАТЬ О.Счет, О.СуммаНачальныйОстаток, О.СуммаКонечныйОстатокКт ИЗ РегистрБухгалтерии.Управленческий.ОстаткиИОбороты(&Н, &К, , , Счет = &Счет) КАК О`
  is compiled
- **THEN** the opening balance sums the records before `&Н`, the closing
  balance every record before `&К`, and the credit part is derived from
  the closing balance

#### Scenario: Auto with a balance
- **WHEN** the table is read under `Авто` with `Организация` and
  `СуммаКонечныйОстаток`
- **THEN** it compiles as the whole interval; reading `Регистратор` as
  well fails with an `UnsupportedFeature` diagnostic

### Requirement: Dereference in virtual-table conditions
A reference path in the condition or the account condition of
`Остатки`, `Обороты` or `ОстаткиИОбороты` of an accumulation or an
accounting register SHALL compile: the target table is joined to the
relation the condition filters with the `LEFT JOIN` and type guard an
ordinary query renders, on the alias of that relation — each side's
branch of a folded accounting table on its own account, the totals and
the movement branch of an accumulation balance on their own base — and
the predicate reads the joined column. A dereference in the access
restriction of such a table SHALL be rendered the same way.

#### Scenario: Account condition through the account's kind
- **WHEN** `РегистрБухгалтерии.Управленческий.Обороты(&Н, &К, , Счет.Вид = ЗНАЧЕНИЕ(ВидСчета.Активный))` is compiled
- **THEN** the debit branch joins the chart on `_AccountDtRRef`, the
  credit branch on `_AccountCtRRef`, and both filter on the chart's
  `_Kind`

#### Scenario: Balance filtered through a parent
- **WHEN** `РегистрНакопления.ЗапасыНаСкладах.Остатки(&Д, Номенклатура.Родитель = &Р)` is compiled
- **THEN** both the totals branch and the movement branch join the
  catalog on their own alias

### Requirement: Running balances of a split table
On PostgreSQL and SQL Server 2012 and newer, `ОстаткиИОбороты` of an
accumulation or an accounting register split by a calendar period, the
recorder, the record, or — under `Авто` — by the split fields the
statement reads, SHALL answer its balance columns as running sums: the
active movements before `Конец` are bucketed by the grain, the
movements before `Начало` forming one bucket that sorts first and is
dropped after the window; `НачальныйОстаток` is the sum of the buckets
before the current one, `КонечныйОстаток` the sum up to it, partitioned
by the dimensions; the debit and credit parts of an accounting balance
are derived from those sums. Under `Авто` the grain SHALL be the record
when `НомерСтроки` is read, the recorder when `Регистратор` or `Период`
is read, otherwise the finest calendar level read; a statement reading
no balance column SHALL keep the relation without windows. On SQL
Server 2008 the balance columns of a split table SHALL be refused with
an `UnsupportedFeature` diagnostic naming the server.

#### Scenario: Balances by recorder
- **WHEN** `ВЫБРАТЬ О.Регистратор, О.КоличествоНачальныйОстаток, О.КоличествоКонечныйОстаток ИЗ РегистрНакопления.Остатки.ОстаткиИОбороты(&Н, &К, Авто, , ) КАК О`
  is compiled for PostgreSQL
- **THEN** the balances are `SUM(SUM(…)) OVER (PARTITION BY <dimensions>
  ORDER BY … ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING)` and `…
  CURRENT ROW` over the record buckets, and the bucket before `&Н` is
  dropped

#### Scenario: Turnovers only
- **WHEN** the same table is read for `ПериодДень` and `КоличествоОборот`
- **THEN** the relation carries no window

#### Scenario: SQL Server 2008
- **WHEN** a split table's balance is read with the `Sql2008` dialect level
- **THEN** compilation fails with an `UnsupportedFeature` diagnostic

### Requirement: Compile the extra-dimension values table
`РегистрБухгалтерии.<Имя>.Субконто` (`ExtDimensions`) SHALL compile as
the register's `_AccRgED` table with `Период`, `Регистратор`,
`НомерСтроки`, `УточнениеПериода`, `ВидДвижения` (`Correspond`, the
side of the record), `Вид` (a reference to the chart of characteristic
types) and `Значение` (a value of several types).

#### Scenario: Values of a record
- **WHEN** `ВЫБРАТЬ С.Вид, С.Значение, С.ВидДвижения ИЗ РегистрБухгалтерии.Хозрасчетный.Субконто КАК С ГДЕ С.Регистратор = &Д` is compiled
- **THEN** the SQL reads `_KindRRef`, the `_Value_*` members and
  `_Correspond` of the register's `_AccRgED` table

### Requirement: Extra dimensions of the aggregating tables
`Остатки`, `Обороты` and `ОстаткиИОбороты` of an accounting register
SHALL expose `Субконто<k>` (`ExtDimension<k>`) and `ВидСубконто<k>`
(`ExtDimensionType<k>`) for `k` up to the register's level count, read
from the side's inline columns (`_ValueDt<k>_*`/`_KindDt<k>RRef` on the
debit side, `Ct` on the credit side) under one name, as dimensions
summed away when unread; `Условие` SHALL see them. Without the
`Субконто` argument the positions are the account's own order. With
the argument — one kind or a parenthesized list of kinds, each
`ЗНАЧЕНИЕ(ПланВидовХарактеристик.…)` or a parameter bound to a
reference — `Субконто<j>` SHALL take the value of whichever level
carries the `j`-th listed kind, and records whose account lacks a listed
kind SHALL be excluded.

#### Scenario: Positional extra dimensions
- **WHEN** `ВЫБРАТЬ О.Счет, О.Субконто1, О.СуммаОстаток ИЗ РегистрБухгалтерии.Хозрасчетный.Остатки(&Д, Счет = &Счет) КАК О` is compiled
- **THEN** each branch reads its side's first value under one name and
  the balance is grouped by account and that value

#### Scenario: Listed kinds
- **WHEN** `Остатки(&Д, , ЗНАЧЕНИЕ(ПланВидовХарактеристик.ВидыСубконтоХозрасчетные.Контрагенты))` is read for `Субконто1`
- **THEN** `Субконто1` is the value of the level whose kind is
  `Контрагенты` on the record's side, and records of accounts without
  that kind are excluded

#### Scenario: Unread extra dimensions
- **WHEN** the statement reads `Счет` and a resource only
- **THEN** the extra dimensions are summed away like any dimension

### Requirement: Compile debit-credit turnovers
`РегистрБухгалтерии.<Имя>.ОборотыДтКт(Начало, Конец, Периодичность,
УсловиеСчетаДт, СубконтоДт, УсловиеСчетаКт, СубконтоКт, Условие)` — the
`Субконто` arguments absent for a register without extra dimensions —
SHALL answer one row per `СчетДт`/`СчетКт` pair, the balance
dimensions, the `Дт`/`Кт` sides of the non-balance dimensions,
`СубконтоДт<k>`/`ВидСубконтоДт<k>`/`СубконтоКт<k>`/`ВидСубконтоКт<k>` in
use and the split of the periodicity, over the active records of
`[Начало, Конец)`, with `<Ресурс>Оборот` per balance resource and
`<Ресурс>ОборотДт`/`<Ресурс>ОборотКт` per non-balance one, unread
dimensions summed away. A listed kind of one side SHALL map that side's
`Субконто<j>` and exclude the records whose account on that side lacks
it. The conditions SHALL see the record's own fields.

#### Scenario: Correspondence with extra dimensions
- **WHEN** `ВЫБРАТЬ О.СчетДт, О.СчетКт, О.СубконтоДт1, О.СуммаОборот ИЗ РегистрБухгалтерии.Хозрасчетный.ОборотыДтКт(&Н, &К, , СчетДт В (&Счета), , , , Организация = &Орг) КАК О`
  is compiled
- **THEN** the SQL groups the main table by both accounts and the debit
  side's first value and sums the balance resource

#### Scenario: Without extra dimensions
- **WHEN** the UNF register is read with six arguments
- **THEN** it compiles, and an eighth argument is a `Syntax` diagnostic

### Requirement: Compile the records with extra dimensions
`РегистрБухгалтерии.<Имя>.ДвиженияССубконто(Начало, Конец, Условие,
Порядок, Первые)` SHALL answer the records of `[Начало, Конец)` with
the main table's fields and `СубконтоДт<k>`, `ВидСубконтоДт<k>`,
`СубконтоКт<k>`, `ВидСубконтоКт<k>` read from the inline columns;
`Условие` SHALL see the same fields; `Порядок` and `Первые` SHALL be
`UnsupportedFeature` diagnostics.

#### Scenario: Records of one contractor
- **WHEN** `ВЫБРАТЬ Д.Регистратор, Д.СубконтоКт1, Д.Сумма ИЗ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, СубконтоКт1 = &Контрагент) КАК Д`
  is compiled
- **THEN** the SQL reads the register's rows with the period bounds and
  the credit side's first value compared

### Requirement: Project expressions of grouped fields
In a statement with `СГРУППИРОВАТЬ ПО`, a projection that is neither a
key nor aggregated SHALL be accepted when every field it reads is named
by a grouping key or the projection is an expression a key spells;
otherwise it SHALL stay an `UnsupportedFeature` diagnostic.

#### Scenario: Negated grouped resource
- **WHEN** `ВЫБРАТЬ Д.Сумма, -Д.Сумма КАК Минус ИЗ … КАК Д СГРУППИРОВАТЬ ПО Д.Сумма`
  is compiled
- **THEN** the SQL groups by the resource column and projects its
  negation

#### Scenario: Ungrouped operand
- **WHEN** a projection reads a field no key names
- **THEN** the diagnostic names the projection

### Requirement: Two-sided condition of the records table
The condition of `РегистрБухгалтерии.<Имя>.ДвиженияССубконто` SHALL
accept, besides the record's fields, the side-less names `Счет`,
`Субконто<k>`, `ВидСубконто<k>` and the names of the non-balance
dimensions and resources; a condition reading any of them SHALL select
the records for which it holds with the debit fields or with the credit
fields substituted. A condition reading none of them SHALL be compiled
once.

#### Scenario: Account of either side
- **WHEN** `ДвиженияССубконто(&Н, &К, Организация = &О И Счет = &С)` is
  compiled
- **THEN** the predicate is the organisation test with the debit account
  test, `OR` the organisation test with the credit account test

#### Scenario: Dereference through a two-sided name
- **WHEN** the condition reads `Счет.Код`
- **THEN** the debit and the credit account are each joined to the chart
  of accounts

### Requirement: Long aliases of nested sources resolve by their text
A field of a nested query or temporary table SHALL resolve by the alias
the text gave the projection, even when the emitted SQL label is
truncated to the provider's limit or suffixed for uniqueness.

#### Scenario: Alias over the PostgreSQL limit
- **WHEN** a nested query projects `… КАК БольничныйЗаСчетРаботодателяСпецРежим`
  and the outer statement reads that alias
- **THEN** the query compiles and the outer projection reads the
  truncated label the nested SELECT emitted

### Requirement: Correspondence of accounting turnovers
`РегистрБухгалтерии.<Имя>.Обороты` SHALL expose `КорСчет`
(`BalancedAccount`), `<Измерение>Кор` (`<Dimension>Balanced`) for each
non-balance dimension and `КорСубконто<k>`/`ВидКорСубконто<k>`
(`BalancedExtDimension<k>`/`BalancedExtDimensionType<k>`): for a debit
row the credit side's values, for a credit row the debit side's. The
`КорСубконто` argument SHALL list the kinds of the correspondence the
way `Субконто` lists the account's, excluding the records whose other
side lacks the kind. `УсловиеКорСчета` SHALL be compiled with the
account condition and the condition, over the same names. An unread
correspondence SHALL be summed away.

#### Scenario: Turnovers with the correspondent account
- **WHEN** `ВЫБРАТЬ О.Счет, О.КорСчет, О.СуммаОборотДт ИЗ РегистрБухгалтерии.Хозрасчетный.Обороты(&Н, &К, , , , , НЕ КорСчет В (&Счета), ) КАК О`
  is compiled
- **THEN** the debit branch projects the credit account as the
  correspondence and tests it against the list, the credit branch the
  debit account, and the outer aggregation groups by both accounts

#### Scenario: Listed balanced kind
- **WHEN** the `КорСубконто` argument lists a kind
- **THEN** each branch picks the value by the opposite side's kinds

### Requirement: Shared names of a derived source fall back to labels
When two or more columns of a nested query or temporary table carry the
same name, each SHALL be addressable by its emitted label — the first
under the name itself, the next under the allocator's suffixed label —
instead of an `AmbiguousField` diagnostic.

#### Scenario: Two unaliased fields of one name
- **WHEN** a nested query projects `Д.Организация, Д.ПодразделениеДт КАК Организация`
  and the outer statement reads `Организация` and `Организация_2`
- **THEN** both resolve to their columns

### Requirement: Compound accounting fields carry member labels
A compound field of an accounting virtual table or record table SHALL
label its columns as `<Имя>_TYPE`, `<Имя>_S`, `<Имя>_N`, `<Имя>_T`,
`<Имя>_L` and `<Имя>` for the reference member, so that a `UNION` branch
projecting a scalar or `НЕОПРЕДЕЛЕНО` in that position is spread over
the same members. The member SHALL be told by the requested name even
when the output label is cut to the dialect's identifier limit.

#### Scenario: Undefined against an extra dimension
- **WHEN** one branch projects `О.Субконто2` of `Обороты` and the other
  `НЕОПРЕДЕЛЕНО`
- **THEN** the union compiles with the second branch spread over the
  members of the first

#### Scenario: Undefined against a long-named composite field
- **WHEN** one branch projects `Д.СубконтоПоАмортизационнойПремии1` and
  the other `НЕОПРЕДЕЛЕНО`, the `_TYPE` label exceeding the PostgreSQL
  identifier limit
- **THEN** the union compiles with the second branch spread over both
  members

### Requirement: Qualified index fields
`ИНДЕКСИРОВАТЬ ПО` SHALL accept a field written as a path
(`Псевдоним.Поле`); the last segment SHALL name the selection-list
label, and a label absent from the selection list SHALL remain a
`TemporaryTable` diagnostic.

#### Scenario: Index field qualified by the source alias
- **WHEN** `ВЫБРАТЬ Т.Код, Т.Наименование ПОМЕСТИТЬ ВТ ИЗ Справочник.Номенклатура КАК Т ИНДЕКСИРОВАТЬ ПО Т.Код, Наименование;`
  is compiled
- **THEN** the statement compiles as with `ИНДЕКСИРОВАТЬ ПО Код, Наименование`

#### Scenario: Qualified field outside the selection list
- **WHEN** `ИНДЕКСИРОВАТЬ ПО Т.Артикул` names a label not projected
- **THEN** the diagnostic is `TemporaryTable`

### Requirement: Index fields by name and by projected path
An index field of `ИНДЕКСИРОВАТЬ ПО` SHALL be accepted when its last
segment equals a column's emitted label or the alias the text gave that
column, or — for a qualified field — when the first branch projects
exactly that field path under any alias.

#### Scenario: Qualified field projected under another alias
- **WHEN** `ВЫБРАТЬ Т.Code КАК Код ПОМЕСТИТЬ ВТ ИЗ … КАК Т ИНДЕКСИРОВАТЬ ПО Т.Code`
  is compiled
- **THEN** the statement compiles

#### Scenario: Unprojected qualified field
- **WHEN** `ИНДЕКСИРОВАТЬ ПО Т.Date` names a field the branch does not
  project
- **THEN** the diagnostic is `TemporaryTable`

### Requirement: Order by a compound field
An `УПОРЯДОЧИТЬ ПО` term naming a field of several columns — by path or
by the alias of its projection — SHALL order by each column in the
field's column order, every column with the term's direction.

#### Scenario: Recorder and extra dimension
- **WHEN** `… УПОРЯДОЧИТЬ ПО Д.Регистратор, Д.СубконтоДт1 УБЫВ` is compiled
- **THEN** the `ORDER BY` lists the recorder's type and reference columns
  ascending, then the extra dimension's type and reference columns
  descending

### Requirement: Accumulation balance and turnovers by recorder
`РегистрНакопления.<Имя>.ОстаткиИОбороты` SHALL accept `Регистратор` and
`Запись` as the periodicity: one row per dimensions combination, record
period and recorder — and line number for `Запись` — with the receipts,
expenses and turnover of the bucket and the opening and closing balances
as running sums over the buckets before it, on a server with window
frames; SQL Server 2008 SHALL refuse the balance columns as for a
calendar periodicity.

#### Scenario: By recorder
- **WHEN** `ОстаткиИОбороты(&Н, &К, Регистратор, , )` is compiled and
  `Регистратор` and `СуммаНачальныйОстаток` are read
- **THEN** the relation groups by the record period and the recorder and
  the opening balance is a window sum over the earlier buckets

### Requirement: Tuple membership with references of several types
In `(<элементы>) [НЕ] В (<подзапрос>)`, an item or a subquery column that
is a reference of several types SHALL be compared as the RTRef ‖ RRRef
payload, the fixed side widened to it; a composite field item SHALL be
compared member by member with a composite projection of the subquery,
its type-reference member included.

#### Scenario: Recorder and line number
- **WHEN** `(Д.Регистратор, Д.НомерСтроки) В (ВЫБРАТЬ П.Регистратор, П.НомерСтроки ИЗ …)` is compiled
- **THEN** the `EXISTS` compares the recorder payloads and the line
  numbers

#### Scenario: Extra dimensions
- **WHEN** `(Д.СубконтоДт1, Д.СубконтоДт2) В (ВЫБРАТЬ П.СубконтоДт1, П.СубконтоДт2 ИЗ …)` is compiled
- **THEN** each item compares its type column and its payload with the
  projection's members

### Requirement: Hierarchy tests in temporary-table definitions
`[НЕ] В ИЕРАРХИИ (…)` SHALL be accepted in a statement with `ПОМЕСТИТЬ`
or `ДОБАВИТЬ`: the recursive CTEs it needs SHALL be defined before the
table's CTE in the `WITH` list of every statement reading the table,
under names unique per table, with `WITH RECURSIVE` on PostgreSQL.

#### Scenario: Temporary table filtered by a hierarchy
- **WHEN** `ВЫБРАТЬ Т.Код ПОМЕСТИТЬ ВТ ИЗ Справочник.Номенклатура КАК Т ГДЕ Т.Ссылка В ИЕРАРХИИ (&Группа); ВЫБРАТЬ ВТ.Код ИЗ ВТ КАК ВТ;`
  is compiled
- **THEN** the final SQL opens with `WITH RECURSIVE`, defines the
  hierarchy CTE, then the table's CTE, and the table's body reads the
  hierarchy by that name

### Requirement: Unaliased fields are labelled as written
A projected field without `КАК` SHALL carry the label of its path
segments after the source alias, as the text spells them and run
together — `Ссылка` for `Т.Ссылка`, `Ref` for `Т.Ref`, `Регистратор`
for `Д.Регистратор`, `ОрганизацияНаименование` for
`Д.Организация.Наименование` — with the member suffixes of a compound
field appended; a nested query or temporary table exposes the column
under that name.

#### Scenario: Temporary table read by the written name
- **WHEN** `ВЫБРАТЬ Д.Регистратор ПОМЕСТИТЬ ВТ ИЗ … КАК Д; ВЫБРАТЬ ВТ.Регистратор ИЗ ВТ КАК ВТ;`
  is compiled
- **THEN** the table's column is `Регистратор` and the second statement
  reads it

#### Scenario: Dereferenced path read by the run-together name
- **WHEN** `ВЫБРАТЬ Д.Организация.Наименование ПОМЕСТИТЬ ВТ ИЗ … КАК Д; ВЫБРАТЬ ВТ.ОрганизацияНаименование ИЗ ВТ КАК ВТ;`
  is compiled
- **THEN** the table's column is `ОрганизацияНаименование` and the
  second statement reads it

### Requirement: Join conditions without an anchor equality
An inner, left or right join SHALL accept any condition over the joined
source and earlier ones — a constant, a comparison with a parameter, an
inequality, `МЕЖДУ`, a dereference through `ВЫРАЗИТЬ` — rendered as the
`ON` predicate; a full join SHALL keep requiring a top-level direct-field
equality between the joined source and an earlier source, reported as
an `UnsupportedFeature` diagnostic naming the full join.

#### Scenario: Left join on a constant
- **WHEN** `… ЛЕВОЕ СОЕДИНЕНИЕ ПланСчетов.Хозрасчетный КАК Х ПО (ИСТИНА)` is compiled
- **THEN** the SQL joins with `ON TRUE`

#### Scenario: Cast dereference in the condition
- **WHEN** the condition reads `ВЫРАЗИТЬ(Д.Регистратор КАК Документ.X).Поле = Т.Поле`
- **THEN** the target of the cast is joined before the condition's join

#### Scenario: Full join without an anchor
- **WHEN** `… ПОЛНОЕ СОЕДИНЕНИЕ … ПО (ИСТИНА)` is compiled
- **THEN** the diagnostic is `UnsupportedFeature`

### Requirement: Value table parameters as sources
`ParameterValue::Table { columns, rows }` SHALL bind a value table whose
columns are `ParameterColumn { name, kind }` with a declared kind: a
string, a number, a boolean, a date, raw bytes, or a reference — to one
object (a 16-byte identifier), to several objects or to none (the
`RTRef ‖ RRRef` payload of a reference of several types). A source
written `&Таблица [КАК Псевдоним]` — after `ИЗ` or a join keyword —
SHALL read that table: the compiler SHALL inline its rows as a common
table expression of the statement, `SELECT 1 AS "__row", CAST(<value>
AS <type>) … UNION ALL SELECT 2, <values>, …`, the first row cast to the
column types and an empty table as a single row of typed `NULL`s with
`WHERE 1 = 0`, and the source SHALL read the CTE by name. Dates SHALL be
rendered in the storage domain. In a statement with
`ПОМЕСТИТЬ`/`ДОБАВИТЬ` the CTE SHALL travel with the table's definition.

#### Scenario: Two rows joined to a catalog
- **WHEN** `ВЫБРАТЬ Т.Код, П.Code ИЗ &Таблица КАК Т ВНУТРЕННЕЕ СОЕДИНЕНИЕ Справочник.X КАК П ПО П.Code = Т.Код`
  is compiled with `Таблица` bound to two rows of a string and a number
- **THEN** the SQL opens with the CTE whose first branch casts `'A'` to
  text and `1` to numeric, the second branch lists the values, and the
  catalog joins the CTE

#### Scenario: Empty table
- **WHEN** the table has no rows
- **THEN** the CTE selects `CAST(NULL AS <type>)` per column with
  `WHERE 1 = 0`

### Requirement: Diagnostics of value table parameters
A parameter read as a source that is unbound or not a table, a table
with uneven rows, a value that does not fit its column's kind, a
reference to an object outside the column's targets, a duplicate or
empty column name, no columns, a column of kind `Null`, `Undefined`,
`Type`, `Uuid` or `Unknown`, or a list or table inside a row SHALL be a
`Parameter` diagnostic at the parameter token; a table used where a
scalar is expected SHALL be a `Parameter` diagnostic as well. The
unbound preparation pass SHALL not fail on such a source: it exposes the
columns the statement names, of no kind.

#### Scenario: Uneven row
- **WHEN** a row has two values for one column
- **THEN** the diagnostic names the row

#### Scenario: Value of another kind
- **WHEN** a string column holds a number in some row
- **THEN** the diagnostic names the row, the column and the kinds

### Requirement: Automatic ordering is accepted
`АВТОУПОРЯДОЧИВАНИЕ` after the keys of `УПОРЯДОЧИТЬ ПО`, or where the
clause would stand, SHALL be accepted and SHALL not change the SQL.

#### Scenario: After the keys
- **WHEN** `… УПОРЯДОЧИТЬ ПО Код АВТОУПОРЯДОЧИВАНИЕ` is compiled
- **THEN** the SQL orders by the code alone

### Requirement: Record auto number
`АВТОНОМЕРЗАПИСИ()` SHALL compile to `ROW_NUMBER() OVER (ORDER BY (SELECT
NULL))` of kind number in any statement; an argument SHALL be a `Syntax`
diagnostic naming zero arguments.

#### Scenario: Numbered projection
- **WHEN** `ВЫБРАТЬ АВТОНОМЕРЗАПИСИ() КАК Номер, Code ИЗ …` is compiled
- **THEN** the first column is the row number

### Requirement: Dereferences of grouping keys
In a statement with `СГРУППИРОВАТЬ ПО`, a projection or a scalar
operand whose path extends a key written as a field path —
`Сотрудник.Наименование` over the key `Сотрудник` — SHALL be accepted,
and every column it reads SHALL be added to the `GROUP BY` list. An
`УПОРЯДОЧИТЬ ПО` key of a grouped statement SHALL be accepted when it is
a grouping key, a dereference of one (its columns joining the grouping)
or an expression containing an aggregate; other unprojected fields SHALL
stay an `UnsupportedFeature` diagnostic.

#### Scenario: Projected dereference
- **WHEN** `ВЫБРАТЬ p.Орг, p.Орг.Код … СГРУППИРОВАТЬ ПО p.Орг` is compiled
- **THEN** the SQL groups by the key column and the joined code column

#### Scenario: Ordering by an aggregate
- **WHEN** `… СГРУППИРОВАТЬ ПО p.Орг УПОРЯДОЧИТЬ ПО МАКСИМУМ(p.Код) УБЫВ` is compiled
- **THEN** the SQL orders by `MAX(...) DESC`

### Requirement: Joined statements order by unprojected fields
A statement with joins and neither a union, a grouping nor `РАЗЛИЧНЫЕ`
SHALL accept an `УПОРЯДОЧИТЬ ПО` field it does not project, ordering by
the field's columns; a projected field SHALL keep ordering by its
position.

#### Scenario: Unprojected field of the joined source
- **WHEN** `ВЫБРАТЬ p.Code ИЗ … КАК p ЛЕВОЕ СОЕДИНЕНИЕ … КАК o ПО … УПОРЯДОЧИТЬ ПО o.Code, p.Code УБЫВ`
  is compiled
- **THEN** the SQL orders by the joined column, then by position 1
  descending

### Requirement: First records of the accounting records table
`РегистрБухгалтерии.<Имя>.ДвиженияССубконто(Начало, Конец, Условие,
Порядок, Первые)` SHALL keep the first `Первые` records — a number
literal or a parameter bound to a number, rendered as `LIMIT` on
PostgreSQL and `TOP (N)` on SQL Server — ordered by `Порядок`: record
fields ascending, singly or as a tuple, or the record order (period,
recorder, line number) when `Порядок` is absent or a parameter bound to
`NULL`. `Порядок` without `Первые` SHALL change nothing. Another
`Первые` or `Порядок` SHALL be an `UnsupportedFeature` diagnostic.

#### Scenario: First record by period
- **WHEN** `ДвиженияССубконто(&Н, &К, , Период, 1)` is compiled for PostgreSQL
- **THEN** the relation ends with `ORDER BY` the period column and `LIMIT 1`

#### Scenario: First five in record order
- **WHEN** `ДвиженияССубконто(&Н, &К, Организация = &О, , 5)` is compiled
- **THEN** the relation orders by period, recorder and line number and
  keeps five rows

### Requirement: Fixed references of derived sources join typed references
A join equality between a column of a nested query or temporary table
that is a reference to one object and a field that is a reference of
several types SHALL compare the field's type discriminator with the
object's database type number found in SchemaStorage, whose table names
carry no leading underscore, and the identifiers.

#### Scenario: Temporary table joined to a recorder
- **WHEN** `… ИЗ (ВЫБРАТЬ Д.Ссылка КАК Ссылка ИЗ Документ.X КАК Д) КАК Т ВНУТРЕННЕЕ СОЕДИНЕНИЕ … КАК Р ПО Р.Регистратор = Т.Ссылка`
  is compiled
- **THEN** the `ON` compares the recorder's type column with the
  document's number and its reference column with the derived column

### Requirement: Expanded balances of the accounting register
`Остатки` SHALL expose `<Ресурс>РазвернутыйОстатокДт` and
`<Ресурс>РазвернутыйОстатокКт`, and `ОстаткиИОбороты` without a
periodicity `<Ресурс>НачальныйРазвернутыйОстатокДт/Кт` and
`<Ресурс>КонечныйРазвернутыйОстатокДт/Кт`: per account, dimensions and
extra dimensions the positive part of the balance and the negated
negative part, which the outer aggregation sums over the dimensions the
statement does not read. A periodic `ОстаткиИОбороты` SHALL refuse them
with an `UnsupportedFeature` diagnostic. The period completion method
SHALL be accepted without a periodicity.

#### Scenario: Expanded balance by account
- **WHEN** `ВЫБРАТЬ О.Счет, О.СуммаРазвернутыйОстатокДт ИЗ РегистрБухгалтерии.Хозрасчетный.Остатки(&Д, , , ) КАК О`
  is compiled
- **THEN** the inner aggregation projects the positive part of the
  group sum and the outer sums it by account

#### Scenario: Periodic table
- **WHEN** the same column is read from `ОстаткиИОбороты(&Н, &К, МЕСЯЦ, , , , )`
- **THEN** the diagnostic is `UnsupportedFeature`

### Requirement: Alias wildcard of the only source
`<Псевдоним>.*` naming a source — by its alias or, without one, by its
object name — SHALL stand for every field of that source in the
metadata order, wherever in the projection list it is written: alone
it equals `*`, and next to named fields or in a joined statement it
adds the source's fields at its place, their labels made unique as any
repeated label is. Any other `<Имя>.*` keeps naming a tabular section.

#### Scenario: Alias wildcard
- **WHEN** `ВЫБРАТЬ Т.* ИЗ Справочник.X КАК Т` is compiled
- **THEN** the SQL equals that of `ВЫБРАТЬ * ИЗ Справочник.X КАК Т`

#### Scenario: Alias wildcard among fields in a join
- **WHEN** `ВЫБРАТЬ Т.Код КАК Код, Т.* ИЗ Справочник.X КАК Т ЛЕВОЕ СОЕДИНЕНИЕ Справочник.Y КАК Д ПО Т.Код = Д.Код`
  is compiled
- **THEN** the projection is `Код` followed by every field of `Т`, the
  repeated `Код` labelled uniquely

### Requirement: Dereferences across targets in expressions
A value read through a reference of several types (`Регистратор.Поле`)
SHALL be usable in an expression as its value member — the `CASE` over
the targets the projection renders — and compared with a reference
constant SHALL compare that member with the constant's `RTRef ‖ RRRef`
payload when the constant's type is known, or with the constant itself
otherwise.

#### Scenario: Filter by the recorder's organisation
- **WHEN** `… ГДЕ П.Регистратор.Организация = ЗНАЧЕНИЕ(Справочник.Организации.ПустаяСсылка)` is compiled
- **THEN** the predicate compares the `CASE` over the recorder's targets
  with the payload of the empty reference

### Requirement: Browse users, roles and their rights in the console
The console SHALL provide `\users` listing the users of the base — name,
description, operating-system login, the e-mail, the show-in-list,
authentication and administrative flags and the role count — `\user
<имя>` showing one user with the names of its roles, `\roles
[<подстрока>]` listing the roles of the configuration by name and
synonym, `\role <имя> [<Вид.Объект>]` listing what the role grants —
every object with its granted rights, or one object with its rights and
restriction texts — and `\rls <Вид.Объект> [<право>]` showing, for the
current user or every role when no user is set, the raw restriction of
each role and the expanded access: `не ограничено`, `запрещено` or the
condition. `\rls` without an object SHALL list the restrictions of the
rights already read — the role, the object and the right of each, and
for a current user only the roles of that user — reading nothing, and
SHALL say which command reads rights when none are read yet. Users and
rights SHALL be read on the first command that needs them through the
read-only query path and forgotten on `\refresh`; an unknown user, role,
object or right SHALL be reported without changing the console state.

#### Scenario: Users listed
- **WHEN** the user enters `\users` on the УНФ demo base
- **THEN** the console prints one line per user with `Абдулов (директор)`
  holding three roles

#### Scenario: Role of a user
- **WHEN** the user enters `\user Абдулов (директор)`
- **THEN** the console prints the user's flags and the role names
  `АдминистраторСистемы`, `ПолныеПрава` and
  `ИнтерактивноеОткрытиеВнешнихОтчетовИОбработок`

#### Scenario: Loaded restrictions listed
- **WHEN** the user enters `\rls` after a command that read the rights of
  a restricting role
- **THEN** the console prints one line per restricted right with the role,
  the object and the right, and no query is sent

#### Scenario: Nothing read yet
- **WHEN** the user enters `\rls` before any rights are read
- **THEN** the console prints no restriction and names the commands that
  read rights

### Requirement: Run allowed queries as a user
`\as <пользователь>` SHALL make that user current and `\as clear` SHALL
forget it; `\as` alone SHALL print the current user. While a user is
current, the console prompt SHALL name it instead of `open-sdbl`, and
the continuation prompt SHALL keep its width; `\as clear` and `\refresh`
SHALL restore `open-sdbl=>`.

`\as <пользователь>` SHALL expand, against the session parameters, the
`Чтение` restrictions every object the user's roles restrict carries, and
SHALL store each expanded condition in the restriction store as a derived
restriction of that object, on one line, leaving the targets a typed
restriction already covers untouched. It SHALL report how many
restrictions it derived and, for the objects whose expansion failed, each
distinct message with the number of objects it applies to, without
failing the command. `\as clear` and `\refresh` SHALL forget the derived
restrictions, and a `\session` command that stores or clears a value
SHALL derive them again while a user is current, reporting as `\as` does,
so a stored condition is never older than the parameters it was expanded
with.

With a current user,
every target a `РАЗРЕШЕННЫЕ` batch requests that no restriction covers
SHALL take the access of the user's roles for `Чтение`, expanded against
the session parameters: no restriction when unrestricted, `ЛОЖЬ` when no
role grants the right, and the restrictions joined by `ИЛИ` otherwise.
A tabular section SHALL take its owner's access as
`Ссылка В (ВЫБРАТЬ <псевдоним>.Ссылка ИЗ <владелец> КАК <псевдоним> ГДЕ <условие>)`.
An expansion error — a session parameter without a value, an outdated
template — SHALL abort the query with the message, naming the role and
the parameter to set with `\session`.

#### Scenario: Restricted table for a user
- **WHEN** `\as Петрова (бухгалтер)` is set and a `РАЗРЕШЕННЫЕ` query
  reads a catalog a role of the user restricts
- **THEN** the query compiles with that role's expanded restriction,
  or reports the session parameter the template needs

#### Scenario: Denied table
- **WHEN** no role of the current user grants `Чтение` of the table
- **THEN** the query compiles with the restriction `ЛОЖЬ` and answers no
  rows

#### Scenario: Prompt names the user
- **WHEN** `\as Абдулов (директор)` is accepted
- **THEN** the prompt reads `Абдулов (директор)=> ` until `\as clear`

#### Scenario: Restrictions derived into the store
- **WHEN** `\as` accepts a user whose roles restrict reading of a catalog
  and the session parameters the templates read have values
- **THEN** `\restrict` lists that catalog with the expanded condition,
  marked as derived, and `\as clear` forgets it

#### Scenario: Restriction that cannot be expanded
- **WHEN** a template of one object reads a session parameter without a
  value
- **THEN** `\as` reports that message with the number of objects it
  applies to, stores the restrictions it could expand, and stays the
  current user

### Requirement: Skip unreadable Config resources
A Config resource the decoder cannot read — one that is not UTF-8 or not
the brace serialization, such as the binary `.7` resource of some charts
of characteristic types — SHALL be skipped with a warning naming it, and
the metadata SHALL be acquired from the rest.

#### Scenario: Binary predefined resource
- **WHEN** a `.7` resource starts with bytes that are not UTF-8
- **THEN** the console warns and the metadata is acquired
