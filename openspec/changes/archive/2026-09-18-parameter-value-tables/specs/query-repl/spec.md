## ADDED Requirements

### Requirement: Value table parameters as sources
`ParameterValue::Table { columns, rows }` SHALL bind a value table. A
source written `&Таблица [КАК Псевдоним]` — after `ИЗ` or a join
keyword — SHALL read that table: the compiler SHALL inline its rows as a
common table expression of the statement, `SELECT 1 AS "__row",
<values> UNION ALL SELECT 2, …` (an empty table as a single `SELECT …
WHERE 1 = 0` of `NULL`s), and the source SHALL read the CTE by name. A
column's kind SHALL be the kind of its values, `NULL` fitting any;
references to one object SHALL stay 16-byte identifiers, references to
several objects SHALL be rendered as the `RTRef ‖ RRRef` payload of a
reference of several types. Dates SHALL be rendered in the storage
domain. In a statement with `ПОМЕСТИТЬ`/`ДОБАВИТЬ` the CTE SHALL travel
with the table's definition.

#### Scenario: Two rows joined to a catalog
- **WHEN** `ВЫБРАТЬ Т.Код, П.Code ИЗ &Таблица КАК Т ВНУТРЕННЕЕ СОЕДИНЕНИЕ Справочник.X КАК П ПО П.Code = Т.Код`
  is compiled with `Таблица` bound to two rows
- **THEN** the SQL opens with the CTE of two `UNION ALL` branches and
  joins the catalog to it

#### Scenario: Empty table
- **WHEN** the table has no rows
- **THEN** the CTE selects `NULL` columns with `WHERE 1 = 0`

### Requirement: Diagnostics of value table parameters
A parameter read as a source that is unbound or not a table, a table
with uneven rows, mixed kinds in a column, a duplicate or empty column
name, no columns, or a list or table inside a row SHALL be a `Parameter`
diagnostic at the parameter token; a table used where a scalar is
expected SHALL be a `Parameter` diagnostic as well. The unbound
preparation pass SHALL not fail on such a source: it exposes the columns
the statement names, of no kind.

#### Scenario: Uneven row
- **WHEN** a row has two values for one column
- **THEN** the diagnostic names the row
