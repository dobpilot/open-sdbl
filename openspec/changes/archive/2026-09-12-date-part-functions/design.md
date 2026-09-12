## Context

Measured on the platform (8.3.27.2342, PostgreSQL 18 demo server,
2026-09-12) with a probe query over twenty boundary dates:

- `ГОД`, `КВАРТАЛ`, `МЕСЯЦ`, `ДЕНЬГОДА`, `ДЕНЬ`, `ЧАС`, `МИНУТА`,
  `СЕКУНДА` are the plain calendar parts; the platform renders them as
  `DATE_PART('YEAR', x)::int::numeric`.
- `ДЕНЬНЕДЕЛИ` is `mod(extract(dow from x)::int + 6, 7) + 1`: Monday 1 …
  Sunday 7.
- `НЕДЕЛЯ` is `(doy - 1 + weekday(1 January) - 1) / 7 + 1` with Monday
  weekdays: 2021-01-03 (Sunday) is week 1, 2021-01-04 is week 2,
  2024-12-30 (Monday) is week 53 of 2024 while 2025-01-01 is week 1. It is
  not ISO 8601.
- The functions nest (`ГОД(КОНЕЦПЕРИОДА(…))`) and serve as `GROUP BY`
  keys.

## Decisions

### AST and parsing

One node `Expression::DatePart { token, part: DatePart, value }` with a
`DatePart` enum of ten variants keeps the match arms small. The ten
keywords are contextual identifiers, so `PeriodKind::from_name` still
reads period names such as `ДЕНЬ` from their lexeme, and aliases and field
names spelled like a function keep parsing. The argument must be a date
field or a `DateTime`-kind expression, as for `НАЧАЛОПЕРИОДА`.

### Rendering

PostgreSQL: `CAST(EXTRACT(<field> FROM v) AS integer)` with `YEAR`,
`QUARTER`, `MONTH`, `DOY`, `DAY`, `HOUR`, `MINUTE`, `SECOND`, and `ISODOW`
for `ДЕНЬНЕДЕЛИ`; `НЕДЕЛЯ` is `((CAST(EXTRACT(DOY FROM v) AS integer) +
CAST(EXTRACT(ISODOW FROM date_trunc('year', v)) AS integer) - 2) / 7 + 1)`.
`ISODOW` exists since PostgreSQL 8.4, within the 9.0 portability target.

MSSQL: `DATEPART(year|quarter|month|dayofyear|day|hour|minute|second,
v)`; `ДЕНЬНЕДЕЛИ` is `((DATEDIFF(day, CONVERT(date, '19000101', 112),
CONVERT(date, v)) % 7 + 7) % 7 + 1)`, independent of `DATEFIRST` because
1900-01-01 is a Monday; `НЕДЕЛЯ` is `((DATEPART(dayofyear, v) - 1 +
((DATEDIFF(day, CONVERT(date, '19000101', 112), CONVERT(date, v)) -
DATEPART(dayofyear, v) + 1) % 7 + 7) % 7) / 7 + 1)`, the same formula with
the weekday of 1 January derived from the day number. Both levels of the
MSSQL dialect share the text. With a non-zero year offset in a
source-backed statement `v` is wrapped as `DATEADD(year, -offset, v)`
first, so `ГОД` returns the logical year and weekdays stay right for any
offset.

The result kind is `Number` and the SQL type is `integer`/`int`, which
the CLI already decodes.

## Risks / Trade-offs

- A `WHERE ГОД(Дата) = 2020` predicate cannot use an index on either
  provider; that is inherent to the function and the same on the platform.
- `EXTRACT(SECOND …)` returns fractional seconds on PostgreSQL; 1C dates
  carry none, and the cast to `integer` would round a fraction.
