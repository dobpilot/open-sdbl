## ADDED Requirements

### Requirement: Select a restricted compilation mode at preparation
Preparation SHALL accept a restriction mode independent of the query text:
`Statement`, the default, restricts the statements that carry
`РАЗРЕШЕННЫЕ`, and `Restricted` restricts every statement of the batch.
The mode SHALL be chosen through prepare-time options, SHALL be stored on
the prepared query, and SHALL be applied by every later compilation of
that prepared query. No value passed at compile time SHALL be able to
lower it. The mode SHALL NOT be implemented by inserting a keyword into
the source text, and the source text SHALL NOT be altered by it.

#### Scenario: A query without the keyword is still protected
- **WHEN** a query that does not carry `РАЗРЕШЕННЫЕ` is prepared in
  `Restricted` mode and compiled with a condition for its source
- **THEN** the generated SQL reads the source through the restricted
  derived table

#### Scenario: Compilation cannot lower the mode
- **WHEN** a query prepared in `Restricted` mode is compiled with default
  compile options
- **THEN** compilation fails because the targets have no decision, rather
  than producing an unrestricted read

#### Scenario: The default mode is unchanged
- **WHEN** a query is prepared without naming a mode
- **THEN** it compiles exactly as it did before, with only
  `РАЗРЕШЕННЫЕ` statements restricted

### Requirement: Demand an explicit access decision for every target
In `Restricted` mode the application SHALL answer every target of the
restriction request with exactly one decision: allowed without a filter,
allowed under a condition, or denied. A target with no decision SHALL be a
`Restriction` diagnostic naming the metadata object and, when the target
is a tabular section, the section name. A denial SHALL render the
restricted derived table with a predicate that admits no row. Two
decisions for one target SHALL be a `Restriction` diagnostic. No failure
in resolving, compiling or applying a decision SHALL fall back to an
unfiltered read.

#### Scenario: Missing decision
- **WHEN** a batch prepared in `Restricted` mode is compiled with
  decisions for all but one target
- **THEN** compilation fails with a `Restriction` diagnostic naming the
  object of the undecided target

#### Scenario: Explicitly allowed without a filter
- **WHEN** a target is answered with the unrestricted decision
- **THEN** the source compiles without a restricted wrapper and the SQL
  matches the unrestricted compilation of the same source

#### Scenario: Denied target
- **WHEN** a target is answered with the denied decision
- **THEN** the source reads through the restricted derived table whose
  predicate is false, so the statement can return no row of that table

#### Scenario: A broken condition does not widen access
- **WHEN** a supplied condition fails to lex, parse, resolve or generate
- **THEN** compilation fails with a `Restriction` diagnostic and no SQL is
  produced

### Requirement: Cover the implicit reads of a restricted compilation
In `Restricted` mode the restriction request SHALL list every metadata
object a statement reads implicitly as well as explicitly: the target of a
reference dereference, every candidate target of a composite reference
hop, and the target of a reference presentation, beside the sources named
by `ИЗ`, joins, nested queries, union branches and virtual tables. A
decision for such a target SHALL be applied to that read: the joined
relation SHALL be the restricted derived table, so rows the decision
excludes contribute no attribute value. One target read through several
aliases SHALL appear once in the request and SHALL be filtered at each
read.

#### Scenario: Dereference of a restricted catalog
- **WHEN** a statement in `Restricted` mode projects
  `Т.Контрагент.Наименование` and the counterparty target is answered with
  a condition
- **THEN** the request lists the counterparty catalog and the generated
  `LEFT JOIN` reads it through the restricted derived table

#### Scenario: Composite reference hop
- **WHEN** the dereferenced field is a composite reference and two of its
  candidate types are read
- **THEN** each candidate type appears in the request and each of its
  joins reads through the restricted derived table

#### Scenario: One target under several aliases
- **WHEN** the same catalog is read as `ИЗ`, as a join, and as a
  dereference target within one statement
- **THEN** the request lists it once and every one of the three reads is
  filtered

### Requirement: Refuse a construct whose restricted read is not implemented
In `Restricted` mode a read of a base table that the compiler cannot
filter SHALL fail with an `UnsupportedFeature` diagnostic positioned at
the construct, before any SQL is generated. At minimum, hierarchy descents
(`В ИЕРАРХИИ` and `ИТОГИ … ПО … ИЕРАРХИЯ`), filter-criterion sources, the
constants source, a nested tabular-section projection, a deferred
reference presentation, and a temporary table whose defining statement was
not compiled in `Restricted` mode SHALL be refused this way. The last two
are refused because their rows travel in a second query the caller runs
itself, which this compilation does not filter. A temporary table
defined inside the same restricted batch SHALL be readable, because its
defining statement was itself filtered. A document journal is an ordinary
source: it SHALL be requested and filtered like any other table.

#### Scenario: Hierarchy descent
- **WHEN** a statement in `Restricted` mode uses `В ИЕРАРХИИ`
- **THEN** compilation fails with an `UnsupportedFeature` diagnostic at
  the construct

#### Scenario: Temporary table of the same batch
- **WHEN** a restricted batch places rows into a temporary table and a
  later statement of the same batch reads it
- **THEN** both statements compile, the defining statement filtered

#### Scenario: Temporary table of an unrestricted batch
- **WHEN** a restricted batch reads a temporary table that an earlier
  unrestricted batch defined
- **THEN** compilation fails with an `UnsupportedFeature` diagnostic at
  the source

#### Scenario: A read the caller resolves separately
- **WHEN** a statement in `Restricted` mode projects a nested tabular
  section, or a reference presentation the compiler would defer
- **THEN** compilation fails with an `UnsupportedFeature` diagnostic
  naming the construct

#### Scenario: Document journal
- **WHEN** a statement in `Restricted` mode reads a document journal
- **THEN** the journal is a target of the request and a decision for it
  filters the read

### Requirement: Keep restriction conditions on the host side of the trust boundary
A restriction condition SHALL be treated as input from the embedding
application, never from the person running the query. The tables a
condition reads through its own nested queries SHALL NOT be filtered
recursively, in `Restricted` mode as in `Statement` mode, and a condition
SHALL see session parameters only. Query text and query parameter values
SHALL NOT reach the compilation path of a condition.

#### Scenario: A condition's own reads stay unfiltered
- **WHEN** a condition of a restricted target reads an access-key register
  through `В (ВЫБРАТЬ …)`
- **THEN** that register is read without a restriction of its own, and is
  not added to the restriction request

#### Scenario: Query parameters stay out of a condition
- **WHEN** a query parameter and a session parameter share a name and the
  condition references it
- **THEN** the condition resolves the session value
