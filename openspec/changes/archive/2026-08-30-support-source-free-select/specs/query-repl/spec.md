## ADDED Requirements

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
