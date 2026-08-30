## ADDED Requirements

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
