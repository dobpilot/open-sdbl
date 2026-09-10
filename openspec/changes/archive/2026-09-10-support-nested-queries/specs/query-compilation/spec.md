## MODIFIED Requirements

### Requirement: Emit unique result column labels
Generated result column labels SHALL be unique within a statement and SHALL
respect the target dialect's identifier length limit, truncating on a UTF-8
character boundary. PostgreSQL limits are measured in UTF-8 bytes and MSSQL
limits in UTF-16 code units. The compiled query's column metadata SHALL match
the labels actually emitted and SHALL pair every label with its column kind.
A nested statement SHALL allocate its own label set, so the same alias MAY
appear in a nested statement and in the statement that projects it.

#### Scenario: Long colliding aliases
- **WHEN** two projection aliases exceed the dialect identifier limit and
  share a truncated prefix
- **THEN** the generated labels remain distinct and the compiled column list
  reports the emitted labels together with their kinds

#### Scenario: Alias reused across nesting levels
- **WHEN** a nested source projects `Сумма` and the outer query projects the
  derived column under the same alias
- **THEN** both statements emit the label and the compiled column list
  reports it once for the outer statement
