## ADDED Requirements

### Requirement: Long aliases of nested sources resolve by their text
A field of a nested query or temporary table SHALL resolve by the alias
the text gave the projection, even when the emitted SQL label is
truncated to the provider's limit or suffixed for uniqueness.

#### Scenario: Alias over the PostgreSQL limit
- **WHEN** a nested query projects `… КАК БольничныйЗаСчетРаботодателяСпецРежим`
  and the outer statement reads that alias
- **THEN** the query compiles and the outer projection reads the
  truncated label the nested SELECT emitted
