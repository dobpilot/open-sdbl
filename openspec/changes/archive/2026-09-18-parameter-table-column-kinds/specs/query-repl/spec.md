## MODIFIED Requirements

### Requirement: Value table parameters as sources
`ParameterValue::Table { columns, rows }` SHALL bind a value table whose
columns are `ParameterColumn { name, kind }` with a declared kind: a
string, a number, a boolean, a date, raw bytes, or a reference — to one
object (a 16-byte identifier), to several objects or to none (the
`RTRef ‖ RRRef` payload of a reference of several types). A source
written `&Таблица [КАК Псевдоним]` — after `ИЗ` or a join keyword —
SHALL read that table: the compiler SHALL inline its rows as a common
table expression of the statement, `SELECT 1 AS "__row", CAST(<value>
AS <type>) … UNION ALL SELECT 2, <values>, …`, the first row cast to the
column types and an empty table as a single row of typed `NULL`s with
`WHERE 1 = 0`, and the source SHALL read the CTE by name. Dates SHALL be
rendered in the storage domain. In a statement with
`ПОМЕСТИТЬ`/`ДОБАВИТЬ` the CTE SHALL travel with the table's definition.

#### Scenario: Two rows joined to a catalog
- **WHEN** `ВЫБРАТЬ Т.Код, П.Code ИЗ &Таблица КАК Т ВНУТРЕННЕЕ СОЕДИНЕНИЕ Справочник.X КАК П ПО П.Code = Т.Код`
  is compiled with `Таблица` bound to two rows of a string and a number
- **THEN** the SQL opens with the CTE whose first branch casts `'A'` to
  text and `1` to numeric, the second branch lists the values, and the
  catalog joins the CTE

#### Scenario: Empty table
- **WHEN** the table has no rows
- **THEN** the CTE selects `CAST(NULL AS <type>)` per column with
  `WHERE 1 = 0`

### Requirement: Diagnostics of value table parameters
A parameter read as a source that is unbound or not a table, a table
with uneven rows, a value that does not fit its column's kind, a
reference to an object outside the column's targets, a duplicate or
empty column name, no columns, a column of kind `Null`, `Undefined`,
`Type`, `Uuid` or `Unknown`, or a list or table inside a row SHALL be a
`Parameter` diagnostic at the parameter token; a table used where a
scalar is expected SHALL be a `Parameter` diagnostic as well. The
unbound preparation pass SHALL not fail on such a source: it exposes the
columns the statement names, of no kind.

#### Scenario: Uneven row
- **WHEN** a row has two values for one column
- **THEN** the diagnostic names the row

#### Scenario: Value of another kind
- **WHEN** a string column holds a number in some row
- **THEN** the diagnostic names the row, the column and the kinds
