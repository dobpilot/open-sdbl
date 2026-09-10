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
`RIGHT [OUTER] JOIN` in source order, each introducing one more source
scope, or contain exactly one `ПОЛНОЕ [ВНЕШНЕЕ] СОЕДИНЕНИЕ` / `FULL
[OUTER] JOIN`. Joined branches SHALL support named direct fields and one-hop
reference properties. Each join condition SHALL contain at least one top-level
scalar direct-field equality between the joined source and an earlier source
and MAY combine that anchor with additional supported scalar direct-field
predicates over the joined source and earlier sources by top-level
`И`/`AND`. Additional predicates SHALL remain in ON. Final `УПОРЯДОЧИТЬ
ПО`/`ORDER BY` SHALL support `ВОЗР`/`ASC` and `УБЫВ`/`DESC`. Statements
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
- **WHEN** a supported FULL JOIN is compiled for PostgreSQL
- **THEN** generated SQL contains two LEFT JOIN branches connected by UNION ALL
  and an IS NULL anti-match predicate, and contains no native FULL JOIN

#### Scenario: FULL JOIN result operators
- **WHEN** a FULL JOIN uses WHERE, DISTINCT, TOP, or final ordering
- **THEN** filtering preserves null-extended row semantics and result-level
  operations apply to the complete transposed result

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
  joined projection, non-scalar join fields, reference properties in ON,
  lacks a top-level direct-field equality with an earlier source, or nests
  its only equality under OR
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

### Requirement: Verify read-only semantics on every provider
Every database session SHALL establish provider-enforced read-only
semantics and verify them server-side before executing user queries; a
failed rollback SHALL poison the session instead of leaving an open
transaction in use.

#### Scenario: MSSQL verification
- **WHEN** a query is executed over an MSSQL session
- **THEN** the session has verified server-side that no stale
  transaction is open before the query runs

#### Scenario: Failed rollback
- **WHEN** a rollback after a failed query itself fails
- **THEN** the session is not reused; the CLI reports the state and
  reconnects or exits

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
input.

#### Scenario: Nested date constructor
- **WHEN** `НАЧАЛОПЕРИОДА` wraps a `ДАТАВРЕМЯ` expression
- **THEN** the nested typed date is truncated to the requested boundary

#### Scenario: Source field boundary
- **WHEN** a source-backed projection or filter applies `НАЧАЛОПЕРИОДА` to a
  date field
- **THEN** generated SQL evaluates the function in the database and preserves
  MSSQL year-offset semantics

#### Scenario: Virtual-table date argument
- **WHEN** a supported register virtual table receives a constant date-function
  expression as its period argument
- **THEN** the expression is compiled in the physical storage date domain

#### Scenario: Unknown period
- **WHEN** the second argument is absent or is not a supported period identifier
- **THEN** compilation returns a positional diagnostic and emits no SQL

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
`СУММА`, `МИНИМУМ`, `МАКСИМУМ`, and `КОЛИЧЕСТВО([РАЗЛИЧНЫЕ] …)` SHALL accept
any scalar expression as their argument. `СУММА` and `КОЛИЧЕСТВО` SHALL
report a number kind; `МИНИМУМ`/`МАКСИМУМ` SHALL report the argument's kind
and SHALL aggregate the payload of a reference expression on both providers.
Aggregates nested in aggregates SHALL fail with a positional diagnostic.

#### Scenario: Conditional sum
- **WHEN** a query projects `СУММА(ВЫБОР КОГДА Вид = ЗНАЧЕНИЕ(…) ТОГДА Сумма ИНАЧЕ 0 КОНЕЦ)`
- **THEN** generated SQL contains `SUM(CASE WHEN … END)` and the column kind
  is number

#### Scenario: Distinct count of an expression
- **WHEN** a query projects `КОЛИЧЕСТВО(РАЗЛИЧНЫЕ НАЧАЛОПЕРИОДА(Дата, МЕСЯЦ))`
- **THEN** generated SQL contains `COUNT(DISTINCT …)` over the period
  expression

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
inline presentations, and final ordering together with `ПЕРВЫЕ`, but SHALL
NOT contain ordering without `ПЕРВЫЕ`, deferred reference presentations, or
`*`. A
derived column whose kind is a fixed single-target reference SHALL support
one-hop dereference through the shared join cache; a runtime-typed derived
column SHALL NOT. Identifiers that resolve only in an enclosing query SHALL
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
