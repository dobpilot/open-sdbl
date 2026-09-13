## ADDED Requirements

### Requirement: Keep a recorded corpus of real queries
The repository SHALL carry the query texts of a real 1C configuration
together with the result the compiler produces for each, and a metadata
fixture of that configuration pruned to the objects those queries reach.
A test SHALL recompile every query against the fixture and compare the
result with the recorded one, failing on any difference, and SHALL assert
the number of queries that compile.

#### Scenario: Unchanged compiler
- **WHEN** the corpus test runs against an unchanged compiler
- **THEN** every query produces its recorded SQL or diagnostic

#### Scenario: Improvement
- **WHEN** a change makes a previously refused query compile
- **THEN** the test fails with the difference, and the recorded result and
  the count are updated in the same commit
