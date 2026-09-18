## ADDED Requirements

### Requirement: Hierarchy tests in temporary-table definitions
`[НЕ] В ИЕРАРХИИ (…)` SHALL be accepted in a statement with `ПОМЕСТИТЬ`
or `ДОБАВИТЬ`: the recursive CTEs it needs SHALL be defined before the
table's CTE in the `WITH` list of every statement reading the table,
under names unique per table, with `WITH RECURSIVE` on PostgreSQL.

#### Scenario: Temporary table filtered by a hierarchy
- **WHEN** `ВЫБРАТЬ Т.Код ПОМЕСТИТЬ ВТ ИЗ Справочник.Номенклатура КАК Т ГДЕ Т.Ссылка В ИЕРАРХИИ (&Группа); ВЫБРАТЬ ВТ.Код ИЗ ВТ КАК ВТ;`
  is compiled
- **THEN** the final SQL opens with `WITH RECURSIVE`, defines the
  hierarchy CTE, then the table's CTE, and the table's body reads the
  hierarchy by that name
