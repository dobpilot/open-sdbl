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
expressions. A branch MAY instead contain one two-source
`[ВНУТРЕННЕЕ] СОЕДИНЕНИЕ` / `[INNER] JOIN`, `ЛЕВОЕ [ВНЕШНЕЕ]
СОЕДИНЕНИЕ` / `LEFT [OUTER] JOIN`, `ПРАВОЕ [ВНЕШНЕЕ] СОЕДИНЕНИЕ` /
`RIGHT [OUTER] JOIN`, or `ПОЛНОЕ [ВНЕШНЕЕ] СОЕДИНЕНИЕ` / `FULL
[OUTER] JOIN`. Joined branches SHALL support named direct fields and one-hop
reference properties with one or more scalar cross-source direct-field
equality conditions combined by `И`/`AND`. Final `УПОРЯДОЧИТЬ ПО`/`ORDER BY`
SHALL support `ВОЗР`/`ASC` and `УБЫВ`/`DESC`. One or more trailing
semicolons SHALL terminate the query. Unsupported syntax SHALL fail before
execution.

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
- **WHEN** a later branch has a different logical or expanded projection width
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

#### Scenario: Unsupported join shape
- **WHEN** a query uses more than one join, wildcard joined projection,
  non-scalar join fields, reference properties in ON, or a condition other
  than cross-source equality conjunctions
- **THEN** compilation returns a positional diagnostic and no SQL is produced

#### Scenario: Repeated query terminator
- **WHEN** a valid query ends in more than one semicolon
- **THEN** all trailing semicolons are consumed as terminators

#### Scenario: Bounded syntax failure
- **WHEN** a query contains a mutation, temporary table, parameter,
  unsupported clause, ambiguous reference target, path deeper than one hop,
  or branch-local ordering before another union
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
SQL or metadata names from the application.

#### Scenario: Source reference presentation
- **WHEN** a query applies `ПРЕДСТАВЛЕНИЕССЫЛКИ` to the source `Ссылка`
- **THEN** the request contains the source object GUID and the generated SQL
  applies its returned plan to the source row without an unnecessary join

#### Scenario: Fixed reference field presentation
- **WHEN** a reference field has one SchemaStorage target
- **THEN** the request contains that target object GUID and generated SQL uses
  one reusable LEFT JOIN to evaluate the target's returned template

#### Scenario: Multiple possible reference targets
- **WHEN** a pure reference field can contain more than one target type
- **THEN** the request contains every target GUID and generated SQL selects the
  corresponding template by the physical RTRef type discriminator

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
`ПРЕДСТАВЛЕНИЕССЫЛКИ`/`REFPRESENTATION`. Their PostgreSQL output SHALL be cast
to text for stable CLI transport. A source-free branch SHALL reject fields,
wildcards, joins, and source-dependent clauses before execution. Source-backed
branches SHALL retain all previously specified projection, source, JOIN,
UNION, filtering, ordering, and diagnostic behavior.

#### Scenario: Source-free numeric literal
- **WHEN** the query is `SELECT 4;`
- **THEN** generated PostgreSQL selects textual value `4` without a FROM clause

#### Scenario: Source-free scalar presentation
- **WHEN** the query is `SELECT ПРЕДСТАВЛЕНИЕ(4);`
- **THEN** generated PostgreSQL selects textual value `4` without requesting a
  reference presentation plan

#### Scenario: Source-free field rejection
- **WHEN** a source-free branch projects an identifier
- **THEN** compilation reports that a field requires FROM and produces no SQL

### Requirement: Compile bounded COUNT projections
The compiler SHALL accept bilingual `COUNT`/`КОЛИЧЕСТВО` projections with
`*`, one resolved field, or `DISTINCT`/`РАЗЛИЧНЫЕ` followed by one resolved
field. It SHALL compile PostgreSQL COUNT and cast the result to text. A pure
compound reference SHALL count its RRef value member. Other compound fields
and COUNT over a transposed FULL JOIN SHALL fail before execution.

#### Scenario: Count all catalog rows
- **WHEN** a query selects `COUNT(*)` from a resolved catalog
- **THEN** PostgreSQL counts all filtered source rows and the CLI receives one
  textual aggregate value

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
PostgreSQL aggregate cast to text. `COUNT(DISTINCT field)` and its Russian form
SHALL remain supported. Wildcard and DISTINCT SHALL be accepted only for COUNT.
All aggregates SHALL share the existing compound-field, projection-mixing, and
transposed FULL JOIN safety checks.

#### Scenario: Numeric sum
- **WHEN** a query selects `СУММА(<numeric-field>)`
- **THEN** generated PostgreSQL applies SUM to the resolved physical column

#### Scenario: Minimum and maximum
- **WHEN** a query selects `MIN(field)` and `МАКСИМУМ(field)`
- **THEN** generated PostgreSQL returns both aggregate values as text

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
