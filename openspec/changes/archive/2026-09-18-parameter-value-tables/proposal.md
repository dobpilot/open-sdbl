## Why

`ИЗ &Таблица` reads a value table the application passes; the compiler
had no parameter of that kind and refused the source. Twenty-five
queries of the demo Бухгалтерия corpus and fourteen of УНФ read such a
table, and the console could not run them at all.

## What Changes

- `ParameterValue::Table { columns, rows }` carries a value table: named
  columns and rows of scalar values.
- `ИЗ &Таблица [КАК Псевдоним]` and a joined `&Таблица` SHALL read the
  table: its rows are inlined as a common table expression of the
  statement — `SELECT 1 AS "__row", <values> UNION ALL SELECT 2, …`, an
  empty table as `SELECT … WHERE 1 = 0` — and the source reads the CTE by
  name; a column's kind is the kind of its values, references to several
  objects widen to the payload of a reference of several types. In a
  `ПОМЕСТИТЬ` statement the CTE travels with the table's definition.
- Malformed tables (uneven rows, mixed kinds, duplicate or empty column
  names, nested lists) and a scalar bound where a table is read SHALL be
  `Parameter` diagnostics at the parameter; a table read as a scalar as
  well.
- The unbound preparation pass SHALL expose, on such a source, the
  columns the statement names.
- The console literal `ТАБЛИЦА(<колонки>)((<строка>), …)` binds a table
  with `\set` and `\session`; the corpus records an empty table with the
  columns a query reads as `T<col>,<col>`.

## Capabilities

### Modified Capabilities

- `query-repl`: value tables as sources.

## Impact

`ParameterValue` gains a variant (the enum is `#[non_exhaustive]`);
`parser.rs`, `select.rs` (`parameter_source_scope`), the console
parameter literal, `tools/corpus/bind_tables.py`.
