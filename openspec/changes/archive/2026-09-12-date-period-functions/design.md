## Context

`НАЧАЛОПЕРИОДА` is a keyword, an `Expression::BeginOfPeriod` node with a
`PeriodKind`, and `SqlDialect::begin_of_period`, which has a separate
`DATEADD`/`DATEDIFF` rendering for SQL Server 2008. Function keywords are
contextual identifiers, so `КАК Год` or a field named `День` keep parsing.
Virtual-table periods accept a literal, `ДАТАВРЕМЯ`, `НАЧАЛОПЕРИОДА` of a
constant, or a top-level `&Параметр`.

The platform semantics were measured on 2026-09-12 with a probe
configuration on the PostgreSQL 18 demo server (platform 8.3.27.2342,
statement log captured through `pg_read_file`), including the SQL the
platform emits:

- `КОНЕЦПЕРИОДА` is the beginning of the next period minus one second;
  `ДЕКАДА` ends on the 10th, the 20th, or the last day of the month.
  Neither `НАЧАЛОПЕРИОДА` nor `КОНЕЦПЕРИОДА` accepts `СЕКУНДА`.
- `ДОБАВИТЬКДАТЕ` accepts `СЕКУНДА`, `МИНУТА`, `ЧАС`, `ДЕНЬ`, `НЕДЕЛЯ`,
  `ДЕКАДА`, `МЕСЯЦ`, `КВАРТАЛ`, `ПОЛУГОДИЕ`, `ГОД`; the count may be a
  field, a parameter, or an arithmetic expression. Month-end days clamp
  (31.01 + 1 month = 29.02.2020, 29.02 + 1 year = 28.02). A fractional
  count is rounded half away from zero for `СЕКУНДА`…`МЕСЯЦ` (1.5 days →
  2, -1.5 → -2) and truncated for `ДЕКАДА`, `КВАРТАЛ`, `ПОЛУГОДИЕ`, `ГОД`
  (1.5 years → 1, 0.7 years → 0), because the platform renders the latter
  as `CAST(n + CASE WHEN n < 0 THEN 0.5 ELSE -0.5 END AS NUMERIC(10, 0))
  * k`.
- `РАЗНОСТЬДАТ` accepts `СЕКУНДА`, `МИНУТА`, `ЧАС`, `ДЕНЬ`, `МЕСЯЦ`,
  `КВАРТАЛ`, `ГОД` (`НЕДЕЛЯ` and `ДЕКАДА` are refused) and counts unit
  boundaries: 31.12.2020 23:59:59 → 01.01.2021 is 1 day, 1 month, 1 year
  and 1 hour, which is what SQL Server's `DATEDIFF` returns. The platform
  renders it on PostgreSQL through truncated-epoch arithmetic and its own
  `DATEDIFF2` PL/pgSQL helper. Differences from 0001-01-01 in seconds
  exceed 32 bits.
- The functions nest freely and are accepted as `GROUP BY` keys.

## Decisions

### AST and parsing

`PeriodKind` gains `Second`. `Expression::EndOfPeriod { token, value,
period }`, `Expression::DateAdd { token, value, period, count }`, and
`Expression::DateDiff { token, from, to, period }` are new nodes. The
period argument is parsed by one helper that takes the function name and
its allowed set: an unknown name is `UnsupportedFeature` (`unsupported
<FN> period "X"`), a known period outside the set is `Syntax` (`<FN> does
not accept the X period`). The first (and for `РАЗНОСТЬДАТ` the second)
argument must be a date field or a `DateTime`-kind expression, as for
`НАЧАЛОПЕРИОДА`; the count of `ДОБАВИТЬКДАТЕ` must be a number-kind
expression, a parameter, or an unknown-kind expression. Depth accounting
follows `parse_begin_of_period`.

### Rendering

Values stay in the domain of the enclosing context: storage on
source-backed statements (the outer projection subtracts the MSSQL year
offset once, as for `НАЧАЛОПЕРИОДА`), logical in source-free statements.
`РАЗНОСТЬДАТ` is a number, so on MSSQL with a non-zero year offset both
operands are shifted to the logical domain with `DATEADD(year, -offset,
…)` before the difference is taken; that keeps day counts exact for any
offset, not only multiples of 400 years.

`КОНЕЦПЕРИОДА` (with `b` the `НАЧАЛОПЕРИОДА` rendering of the same
period):

| period | PostgreSQL | MSSQL |
| --- | --- | --- |
| minute, hour, day, week, month, year | `(b + INTERVAL '1 <unit>' - INTERVAL '1 second')` | `DATEADD(second, -1, DATEADD(<unit>, 1, b))` |
| quarter | `INTERVAL '3 months'` | `DATEADD(quarter, 1, b)` |
| half-year | `INTERVAL '6 months'` | `DATEADD(month, 6, b)` |
| ten days | `LEAST(b + INTERVAL '10 days', date_trunc('month', v) + INTERVAL '1 month')` | `CASE WHEN DAY(v) <= 20 THEN DATEADD(day, 10, b) ELSE DATEADD(month, 1, <month begin>) END` |

The SQL Server 2008 level reuses its `НАЧАЛОПЕРИОДА` emulation for `b`.

`ДОБАВИТЬКДАТЕ` with count `n`: PostgreSQL `(v + CAST(n AS integer) *
INTERVAL '1 <unit>')` (`CAST` of a numeric rounds half away from zero) and
`CAST(trunc(n) AS integer)` for the truncating periods; units are
`second`, `minute`, `hour`, `day`, `7 days`, `10 days`, `1 month`, `3
months`, `6 months`, `1 year`. MSSQL `DATEADD(<unit>, CONVERT(int,
ROUND(n, 0)), v)` and `ROUND(n, 0, 1)` for the truncating periods, with
`day` × 10 for `ДЕКАДА`, `quarter`, `month` × 6, and `year`.

`РАЗНОСТЬДАТ(a, b, unit)`: PostgreSQL `CAST(EXTRACT(EPOCH FROM (b - a)) AS
bigint)` for seconds, the same over `date_trunc('minute'|'hour', …)`
divided by 60 or 3600 for minutes and hours, `(CAST(b AS date) - CAST(a AS
date))` for days, and `CAST((EXTRACT(YEAR FROM b) - EXTRACT(YEAR FROM a))
* 12 + EXTRACT(MONTH FROM b) - EXTRACT(MONTH FROM a) AS integer)` for
months (× 4 with `QUARTER` for quarters, years alone for years). MSSQL
`DATEDIFF(day|month|quarter|year, a, b)`; seconds, minutes, and hours are
`DATEDIFF(day, CONVERT(date, a), CONVERT(date, b)) * CAST(86400 AS bigint)
+ DATEDIFF(second, CONVERT(date, b), b) - DATEDIFF(second, CONVERT(date,
a), a)` (1440/`minute`, 24/`hour`), so no `DATEDIFF_BIG` is needed and the
2008 level is covered. The result kind is `Number` on both providers.

### Virtual-table periods

`compile_constant_date_expression` takes the bound parameters and accepts
`DateTime`, a date `Parameter`, `BeginOfPeriod`/`EndOfPeriod` over an
accepted expression, and `DateAdd` whose count is a numeric literal or a
numeric parameter. `Обороты(НАЧАЛОПЕРИОДА(&П, МЕСЯЦ), КОНЕЦПЕРИОДА(&П,
МЕСЯЦ))` therefore compiles, in the storage domain as before.

### GROUP BY, joins, aggregates

The new nodes join the expression fingerprint used to match `GROUP BY`
keys with projections, the join-scope walk, and the aggregate-containment
walk, so they behave like `НАЧАЛОПЕРИОДА` everywhere an expression is
allowed.

## Risks / Trade-offs

- On MSSQL with a year offset that is not a multiple of 400, adding months
  or years in the storage domain may clamp 29 February differently from
  the logical date; the platform uses only 0 and 2000, and `НАЧАЛОПЕРИОДА`
  already has this property.
- `ДОБАВИТЬКДАТЕ` past 9999-12-31 raises a database error on both
  providers, as on the platform.
- The count rounding mirrors the platform's inconsistency on purpose;
  the docs table names it so readers are not surprised.
