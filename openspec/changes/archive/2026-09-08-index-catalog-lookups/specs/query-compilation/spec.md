## MODIFIED Requirements

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
