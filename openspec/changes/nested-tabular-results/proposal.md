## Why

A tabular section projected as a column is the largest gap left: 29 of the
382 recorded corpus queries stop there, more than every other gap together.
`ВЫБРАТЬ Д.Ссылка, Д.Товары ИЗ Документ.Продажа КАК Д` asks for the rows of
`Товары` inside one cell of each document row, which one SQL statement
cannot return.

The platform does not return it in one statement either. Measured on the
probe base against 8.3.27 with statement logging on, it runs the main
statement, materializes the owner keys into a temporary table
`pg_temp.ttN` with the columns `_TTC_1` (owner reference), `_TTC_1_0` (data
separator) and `SDBL_IDENTITY` (the position of the main row), and then
runs a second statement:

```sql
SELECT <section columns>, T.SDBL_IDENTITY
FROM _Document73_VT75 S INNER JOIN pg_temp.tt1 T
  ON T._TTC_1 = S._Document73_IDRRef AND T._TTC_1_0 = S._Fld58
ORDER BY SDBL_IDENTITY
```

`Д.Товары` selects the owner reference, the line number and every
attribute; `Д.Товары.(НомерСтроки, Товар)` selects exactly the named
columns and no owner reference.

Both planned consumers need the same shape. The `Запрос` object of open-bsl
must answer `Выборка.Товары.Выбрать()`, and the Trino connector builds its
own plan and must not parse our SQL — so the link between the statements
has to be described as data, not embedded in text only.

## What Changes

- Compile `Состав` as a projection, `Состав.(Поле, …)` and `Состав.*` into
  a main statement plus one nested result per section.
- Extend `CompiledQuery` with those nested results: each carries its own
  SELECT-only statement, its columns, and the structural link — which
  column of the main result holds the owner key and which column of the
  nested result matches it.
- Add the owner key to the main statement as a service column when the
  query does not already select it, and mark it as service so a consumer
  does not print it.
- Make `CompiledQuery` `#[non_exhaustive]`, so later result-shape work
  does not break consumers again.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `query-compilation`: one compiled query may carry nested results.
- `query-repl`: a tabular section may be projected.

## Impact

`CompiledQuery` gains fields and becomes `#[non_exhaustive]`; code that
constructs it outside this crate must change, code that reads it need not.
The CLI prints a nested column as its row count. 29 corpus queries compile.
