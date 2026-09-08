## ADDED Requirements

### Requirement: Compile reference UUID expressions
The compiler SHALL accept bilingual `УНИКАЛЬНЫЙИДЕНТИФИКАТОР`/`UUID` with
exactly one field argument that resolves to a reference member and SHALL
compile it with pure SQL into PostgreSQL `uuid` or MSSQL `uniqueidentifier`
in canonical 1C field order, reporting the column kind as UUID. `NULL`
references SHALL yield `NULL`. The expression SHALL be usable wherever scalar
expressions are, including projections and predicates.

#### Scenario: Source reference
- **WHEN** a query projects `УНИКАЛЬНЫЙИДЕНТИФИКАТОР(Ссылка)` from a catalog
- **THEN** PostgreSQL SQL reorders the `_IDRRef` bytes with `substring` and
  casts the hex text to `uuid`, and MSSQL SQL reverses the first three groups
  and casts to `uniqueidentifier`

#### Scenario: Known GUID
- **WHEN** the physical reference bytes are `9022249e3a1ac4b94be8faddd2f8bde9`
- **THEN** both databases return `d2f8bde9-fadd-4be8-9022-249e3a1ac4b9`

#### Scenario: Dereferenced and compound arguments
- **WHEN** the argument is `Ссылка.Владелец` or a compound field with an
  `RRRef` member
- **THEN** the dereference join is reused and only the `RRRef` member is
  decoded

#### Scenario: Invalid argument
- **WHEN** the argument is a non-reference field, a literal, a `ЗНАЧЕНИЕ`
  expression, or the query has no FROM
- **THEN** compilation returns a positional diagnostic and emits no SQL
