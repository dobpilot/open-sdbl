## ADDED Requirements

### Requirement: Filter the deferred presentation lookup
The batch that resolves deferred reference presentations SHALL accept
access decisions and session parameters and apply the decision of its
target to the table it reads, rendering it the way a filtered source of a
statement is rendered. A lookup driven from a prepared query SHALL be
compiled under that query's restriction mode.

In `RestrictionMode::Restricted` a lookup whose target has no decision
SHALL fail with a `Restriction` diagnostic naming the object, and a denied
target SHALL be read through a predicate that admits no row, so a
reference the decision excludes comes back with no presentation rather
than an error or a value. The entry point that takes no decisions SHALL
keep its signature and read unfiltered, and SHALL say so.

#### Scenario: A filtered lookup
- **WHEN** a lookup is compiled with a condition for its target
- **THEN** it reads the target through the restricted derived table and
  answers presentations only for the rows the condition admits

#### Scenario: A reference the decision excludes
- **WHEN** the target is denied and a lookup is asked for a reference of
  it
- **THEN** the statement returns no row for that reference, so the
  application presents nothing for it

#### Scenario: No decision in the restricted mode
- **WHEN** a lookup is compiled in `Restricted` with no decision for its
  target
- **THEN** compilation fails with a `Restriction` diagnostic naming the
  object

#### Scenario: The unrestricted entry point
- **WHEN** an application calls the lookup that takes no decisions
- **THEN** it compiles the statement it compiled before, unfiltered

## MODIFIED Requirements

### Requirement: Refuse a construct whose restricted read is not implemented
In `Restricted` mode a read of a base table that the compiler cannot
filter SHALL fail with an `UnsupportedFeature` diagnostic positioned at
the construct, before any SQL is generated. At minimum, hierarchy descents
(`В ИЕРАРХИИ` and `ИТОГИ … ПО … ИЕРАРХИЯ`), filter-criterion sources, the
constants source, a nested tabular-section projection, and a temporary
table whose defining statement was not compiled in `Restricted` mode SHALL
be refused this way. The nested projection is refused because its rows
travel in a second query the caller runs itself, which this compilation
does not filter.

A deferred reference presentation SHALL NOT be refused: the lookup that
resolves it is filtered in its own right, so deferring reads nothing past
the decisions of the compilation.

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
  section
- **THEN** compilation fails with an `UnsupportedFeature` diagnostic
  naming the construct

#### Scenario: A universal reference presentation
- **WHEN** a statement in `Restricted` mode presents a reference whose
  targets the compiler defers
- **THEN** the statement compiles and reports the deferred presentation,
  for the application to resolve with a filtered lookup

#### Scenario: Document journal
- **WHEN** a statement in `Restricted` mode reads a document journal
- **THEN** the journal is a target of the request and a decision for it
  filters the read
