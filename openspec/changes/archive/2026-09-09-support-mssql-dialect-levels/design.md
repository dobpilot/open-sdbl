## Context

An inventory of `src/query/core/dialect.rs` and the codegen modules found a
single SQL Server 2012 dependency (`DATETIME2FROMPARTS` in eight
`НАЧАЛОПЕРИОДА` arms; the week arm already uses `DATEADD`/`DATEDIFF`) and a
single PostgreSQL 9.4 dependency (`FILTER (WHERE …)`). Acquisition queries are
2005+/8.4+ except the PostgreSQL catalog statement.

## Decisions

### Named levels, not version numbers

`MsSqlDialectLevel` names capability sets the compiler branches on. The CLI
maps `SERVERPROPERTY('ProductVersion')` (`10.50.…` → `Sql2008`, `11.…` and
above → `Sql2012`) to a level; `compatibility_level` is ignored because it
does not gate the built-in functions in question. `ProductMajorVersion` is
not used: it returns NULL on 2008. The enum is `#[non_exhaustive]` so 2005 or
newer levels can be added without a breaking change.

### Builder on the existing backend value

`MsSqlBackend::new(year_offset)` stays and defaults to `Sql2012`, matching
previous output byte for byte; `with_dialect_level` is a `const` builder and
`Default`/`Copy`/`Eq` are preserved. The level travels in
`SqlDialect::MsSql { year_offset, dialect_level }`; existing match arms use
`{ .. }` and are unaffected.

### Emulation formulas (`Sql2008`)

With `B = CONVERT(datetime2, '00010101', 112)` and
`D = CONVERT(datetime2, CONVERT(date, v))`:

| Period | SQL |
|---|---|
| minute / hour | `DATEADD(minute, DATEDIFF(minute, D, v), D)` (difference stays within a day, no `int` overflow) |
| day | `D` |
| week | unchanged |
| ten days | `DATEADD(day, CASE WHEN DAY(v) <= 10 THEN 0 WHEN DAY(v) <= 20 THEN 10 ELSE 20 END, DATEADD(month, DATEDIFF(month, B, v), B))` |
| month / quarter / year | `DATEADD(<part>, DATEDIFF(<part>, B, v), B)` |
| half-year | `DATEADD(month, (DATEDIFF(month, B, v) / 6) * 6, B)` |

`B` is `datetime2`, not `datetime`, so bases with `_YearOffset = 0` and the
1C empty date `0001-01-01` stay in range. Results are `datetime2` on both
levels, so the year-offset wrapper and comparisons are unchanged.

### PostgreSQL stays stateless

Only `FILTER` needed a version switch; the `CASE` form is semantically equal
and already used for MSSQL, so it becomes the single form. The catalog
acquisition statement gains a legacy variant that lists index columns through
a correlated `ARRAY(SELECT … FROM generate_subscripts(x.indkey, 1) …)`
subquery (8.4+); the adapter picks it when `server_version_num < 90400`.

## Risks / Trade-offs

- SQL Server 2005 (no `datetime2`) remains unsupported.
- The PostgreSQL legacy catalog variant is verified against a modern server
  for identical output; a 9.x server is not available for a live run.
