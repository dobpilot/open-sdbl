## Context

Measured on the platform (8.3.27, probe base, 2026-09-13) through the
linear traversal with `ТипЗаписи()` and `Уровень()` recorded per row:

- The overall row comes first, then for each control-point value one
  total row followed by its rows. Groups are ordered by the first
  appearance of their value in the detail result ordered by `УПОРЯДОЧИТЬ
  ПО` (`… ПО Цена УБЫВ ИТОГИ … ПО Родитель` listed the parent of the
  most expensive item first), nested levels likewise; detail rows keep
  the user order inside their group. A `NULL` group is an ordinary
  group.
- A total row carries the control points of its own and enclosing
  levels, `NULL` in the other columns, and the totals fields written into
  the result column they name: a bare aggregate targets its argument
  column, an expression needs `КАК <колонка>`, an alias naming no result
  column fails (`Невозможно определить поле для записи результата`), and
  several fields naming one column let the last win. `КОЛИЧЕСТВО(Имя)`
  writes a number into a string column.
- `ИТОГИ ПО Родитель` without fields gives total rows with `NULL` in
  every non-control-point column.
- `Уровень()`: overall `0`, then `1…n`, details `n + 1`; without
  `ОБЩИЕ` totals start at `0` and details are `n`.
- `ПЕРИОДАМИ(МЕСЯЦ, …)` changed nothing in the linear result: no empty
  periods, and a raw date control point is still grouped by its exact
  value.
- `ПЕРВЫЕ 3` and `ОБЪЕДИНИТЬ` feed the totals with the final rows;
  `ИТОГИ` with `ПОМЕСТИТЬ` is refused by the platform.
- A control point that is not a result column (`ПО Т.Родитель`) works on
  the platform; here it is `UnsupportedFeature` for now.

## Decisions

### Parsing

`QueryAst.totals: Option<TotalsAst>` is parsed after `УПОРЯДОЧИТЬ ПО`
and `ИНДЕКСИРОВАТЬ ПО`. A totals field is a scalar expression over
aggregates (`СУММА`, `СРЕДНЕЕ`, `МИНИМУМ`, `МАКСИМУМ`, `КОЛИЧЕСТВО`,
`КОЛИЧЕСТВО(РАЗЛИЧНЫЕ …)`), arithmetic, literals, and parameters, whose
aggregate arguments are result column names (label, alias, or field
name). A control point is a result column name with an optional
`[ТОЛЬКО] ИЕРАРХИЯ` or `ПЕРИОДАМИ(<period>[, <дата>[, <дата>]])` and an
optional alias (accepted, unused).

### Rendering

The statement without totals is compiled with two changes: its
`УПОРЯДОЧИТЬ ПО` keys are also projected as hidden columns
`__order_1…k` (positional keys reuse the output label), and the `ORDER
BY` itself is emitted only when `ПЕРВЫЕ` needs it. The wrapper is

```sql
WITH "__totals_rows" AS (
  SELECT <labels>, ROW_NUMBER() OVER (ORDER BY <order keys>) AS "__rn"
  FROM (<statement>) AS "__totals_source")
SELECT <labels>[, "__level"] FROM (
  SELECT CAST(NULL AS t1) AS c1, …, SUM("Цена") AS "Цена", …, 0 AS "__level", 0 AS "__g1", …, 0 AS "__rn"
    FROM "__totals_rows"                                   -- ОБЩИЕ
  UNION ALL SELECT cp1, CAST(NULL AS t2) …, SUM("Цена") …, 1, MIN(MIN("__rn")) OVER (PARTITION BY cp1), 0, …, 0
    FROM "__totals_rows" GROUP BY cp1                     -- level 1
  UNION ALL SELECT cp1, cp2, …, 2, MIN(MIN("__rn")) OVER (PARTITION BY cp1), MIN(MIN("__rn")) OVER (PARTITION BY cp2), 0
    FROM "__totals_rows" GROUP BY cp1, cp2                -- level 2 …
  UNION ALL SELECT <labels>, n + 1, MIN("__rn") OVER (PARTITION BY cp1), MIN("__rn") OVER (PARTITION BY cp2), "__rn"
    FROM "__totals_rows"                                  -- details
) AS "__totals"
ORDER BY "__g1", CASE WHEN "__level" <= 1 THEN 0 ELSE 1 END, "__g2", CASE WHEN "__level" <= 2 THEN 0 ELSE 1 END, …, "__rn"
```

`ROW_NUMBER` over the user order numbers the detail rows; a group sorts
at the smallest row number of the rows holding its own value anywhere in
the result (`PARTITION BY` the single control point, not the prefix),
which is what the platform does: in the two-level probe the client
groups inside every parent followed the global first appearance of the
client, not the first appearance inside the parent; the `CASE` flags put
a total before the rows it covers. Without `УПОРЯДОЧИТЬ ПО` the row
numbers follow `ORDER BY (SELECT 1)`, that is the database's order, as
the platform's result would. `NULL` placeholders are typed with the
derived catalog type of the column. A totals field is compiled over the
CTE columns: `SUM("Цена")`, `COUNT(DISTINCT "Клиент")`; a count or sum
written into a string column is cast to text, any other kind mismatch
is a `Syntax` diagnostic. Deferred presentation positions are
unchanged, and total rows carry `NULL` references there.

The CTE joins the batch `WITH` list: `place_statement` merges a
statement that starts with `WITH` into the temporary-table prefix.

### Ordering by aliases

Totals queries habitually order by projection aliases (`УПОРЯДОЧИТЬ ПО
Имя`), which a plain branch refused before (`field "Имя" was not found
in source`). `compile_order_terms` now resolves an alias of the branch in
every mode: positional branches keep their position, a plain branch
orders by the aliased expression.

### Level column

`CompileOptions::totals_level(true)` appends `"__level"` (`Number`) after
the statement's own columns; the console passes it and shows the column
as any other. Without the option the total rows are distinguishable
only by their `NULL` pattern, as in a 1C value table.

## Risks / Trade-offs

- The detail statement is read once through the CTE but its rows are
  scanned once per totals level; acceptable for report-sized results.
- Aggregates over reference columns (`МАКСИМУМ(Ссылка)`) compile to
  `MAX(bytea)`, which needs the platform-created aggregate on
  PostgreSQL, the same dependency the constants table has.
