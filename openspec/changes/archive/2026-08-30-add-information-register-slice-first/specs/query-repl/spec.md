## ADDED Requirements

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

