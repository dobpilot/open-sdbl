## ADDED Requirements

### Requirement: Accept a zero row count
`ПЕРВЫЕ 0`/`TOP 0` SHALL be accepted like any other count and SHALL
render as `LIMIT 0` on PostgreSQL and `TOP (0)` on SQL Server, so the
statement answers its columns and no rows, as on the platform.

#### Scenario: Empty temporary table
- **WHEN** `ВЫБРАТЬ ПЕРВЫЕ 0 Т.Ссылка КАК Ссылка ПОМЕСТИТЬ ВТ ИЗ Справочник.Номенклатура КАК Т` is compiled
- **THEN** the PostgreSQL text ends the temporary table's statement with
  `LIMIT 0` and no diagnostic is reported
