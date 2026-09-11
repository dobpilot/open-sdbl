## ADDED Requirements

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
