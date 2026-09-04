## ADDED Requirements

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
through repetition.

#### Scenario: Pathological repetition
- **WHEN** a query multiplies many union branches over sources whose
  field resolution is expensive
- **THEN** compilation either completes promptly or fails fast with a
  typed work-budget diagnostic
