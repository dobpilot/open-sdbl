## ADDED Requirements

### Requirement: Materialize temporary tables through an opt-in host callback
The compiler SHALL offer a batch path that compiles statements in order and invokes the host once for each `ПОМЕСТИТЬ` or `ДОБАВИТЬ` selection. The callback SHALL receive SQL and projected column kinds and return typed rows. The manager SHALL retain those rows so later statements do not reread the source. A final placement SHALL report the number of rows returned by that one callback; a final ordinary selection SHALL return SQL for host execution; a final `УНИЧТОЖИТЬ` SHALL return no result. Existing `compile_batch` calls SHALL retain their CTE-only behavior.

#### Scenario: Source changes after placement
- **WHEN** a host returns a row `Before` for `ПОМЕСТИТЬ ВТ`, changes the source row to `After`, then compiles a read of `ВТ`
- **THEN** that read uses a literal `Before` row and never contains the source table query

#### Scenario: Dependent placement and append
- **WHEN** a batch places `ВТ1`, places `ВТ2` from `ВТ1`, and appends a further row to `ВТ2`
- **THEN** the host receives one selection per placement or append, in order, and the resulting `ВТ2` contains only the rows returned by those selections

#### Scenario: Drop, redefinition, and failure
- **WHEN** a batch drops `ВТ`, then reads it before redefining it
- **THEN** compilation fails with `TemporaryTable` and the manager remains as it was before the failed batch

#### Scenario: Typed empty and null rows
- **WHEN** the callback returns no rows or a `NULL` under a supported projected column kind
- **THEN** the retained relation preserves the declared column type and cardinality for both SQL dialects

#### Scenario: Invalid or excessive host rows
- **WHEN** the callback returns a row of the wrong width, a value incompatible with its column kind, an unsupported column kind, or more retained bytes than the manager budget
- **THEN** compilation fails with `TemporaryTable` without committing the batch's manager changes

#### Scenario: Later missing session parameter
- **WHEN** the first statement places rows but a later statement requires an absent session parameter
- **THEN** the compiler reports the missing parameter before invoking the placement callback and leaves the manager unchanged

#### Scenario: Host read fails
- **WHEN** the placement callback reports a database error
- **THEN** that error is propagated unchanged and the manager remains unchanged

#### Scenario: SQL Server date offset
- **WHEN** SQL Server uses a nonzero `_YearOffset` and a placement projects a date
- **THEN** the callback SQL projects the logical 1C date, while later SQL stores the value in the physical date domain
