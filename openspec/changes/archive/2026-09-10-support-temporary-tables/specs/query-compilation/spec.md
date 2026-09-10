## ADDED Requirements

### Requirement: Diagnose temporary-table failures
`QueryDiagnosticKind` SHALL include a `TemporaryTable` variant reported
with the offending token for an unknown, hidden, or duplicate temporary
table name, an `ДОБАВИТЬ` structure mismatch, an index field outside the
selection list, a `TempTablesManager` bound to another dialect or holding
its maximum of 256 definitions, and a batch that returns no rows when
compiled through an entry point that must return SQL. Callers matching the
non-exhaustive enum SHALL keep their fallback arm.

#### Scenario: Unknown temporary table
- **WHEN** a statement reads `ИЗ ВТ` and no visible definition named `ВТ`
  exists
- **THEN** the diagnostic kind is `TemporaryTable` and its position is the
  `ВТ` token

#### Scenario: Duplicate definition
- **WHEN** a batch places `ВТ` twice without dropping it in between
- **THEN** the diagnostic kind is `TemporaryTable` at the second name token
