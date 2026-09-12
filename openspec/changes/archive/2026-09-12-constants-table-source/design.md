## Context

`MetadataKind::Constant` objects resolve like any other object: the
physical table is `_Const<N>`, the value column `_Fld<N>` (or the
`_Fld<N>RRef`/`_Fld<N>_TYPE` pair) is exposed under the constant's own
name, and `_RecordKey` is the platform's fixed-key column. `Константы` is
listed as unsupported in `docs/query-language-support.md`.

Aggregation is the maintainer's choice over a multi-way join or scalar
subqueries. Measured on the demo base (PostgreSQL 18, platform 8.3.27):
`max(numeric)`, `max(text)`, `max(timestamp)` are built in;
`max(bytea)` is built in on PostgreSQL 18 and is also created by the
platform in `public` together with `max(boolean)`, `max(mchar)`, and
`max(mvarchar)`, so every 1C storage type on PostgreSQL has a `MAX`. On
SQL Server the 1C storage types are `numeric`, `nvarchar`/`nchar`,
`datetime2`, `binary(1)` for booleans, and `binary(16)` for references;
`MAX` covers all of them (`bit`, which `MAX` rejects, is not used).

## Decisions

### Source shape

`Константы` (`Constants`) is a new `SourceKind::Constants` resolved without
a metadata object. Its queryable fields are built from every
`MetadataKind::Constant` object with a live physical table, named by the
constant's name, typed exactly as the single-constant source types them.
No `Ссылка`, no `Представление`; `ПРЕДСТАВЛЕНИЕ(Константы.Организация)`
works through the reference-typed field as for any other reference field.

### Rendering

Only constants referenced by the statement (projection, predicates,
dereferences, ordering) are read; `*` reads all live ones. With constants
`A` (`_Const10`, number) and `B` (`_Const11`, reference):

```sql
(SELECT MAX("__constants"."_Fld10") AS "_Fld10",
        MAX("__constants"."_Fld11RRef") AS "_Fld11RRef"
 FROM (SELECT "__constant"."_Fld10" AS "_Fld10", CAST(NULL AS bytea) AS "_Fld11RRef"
       FROM "_Const10" AS "__constant" WHERE "__constant"."_Fld999" = 0
       UNION ALL
       SELECT CAST(NULL AS numeric(10,2)) AS "_Fld10", "__constant"."_Fld11RRef" AS "_Fld11RRef"
       FROM "_Const11" AS "__constant" WHERE "__constant"."_Fld999" = 0) AS "__constants") AS "К"
```

An aggregate query without `GROUP BY` yields exactly one row, which is the
platform semantics for `Константы`; an unwritten constant contributes no
branch row and surfaces as `NULL`. A statement that reads no constant
field (`КОЛИЧЕСТВО(*)`) gets the one-row stand-in `(SELECT 1 AS
"__constants_row")`. The scope records which of its fields the statement
resolved (`SourceScope::used_fields`, filled by every path resolution and
by `*`), and the relation is rendered after the branch is compiled
(`finalize_constants_relation`). `Константы` takes precedence over a
temporary table of the same name. The `NULL` placeholders are cast to the
column's catalog type (`format_type` on PostgreSQL, the `sys.types`
spelling on SQL Server): PostgreSQL resolves a `UNION` column whose first
branches are untyped `NULL` as `text` and then rejects the `bytea` branch,
as the demo base showed. The separator predicate is the one
`data-separator-predicates` computes for the statement; a disabled
separator on a referenced constant is an `UnsupportedFeature` diagnostic at
the source token: `constant "<name>" is separated; the constants table
needs a separator value`.

`_RecordKey` is added to the PostgreSQL recase tokens so `\d` output and
column lookups spell it as the platform does.

### Column kinds

Output `ColumnKind`s of the derived table reuse the single-constant
mapping per column, so `\dt`-style formatting and `UNION` compatibility
checks behave as for `Константа.Имя`.

## Risks / Trade-offs

- `MAX` on PostgreSQL relies on the platform-created `public` aggregates
  for `boolean`, `mchar`, `mvarchar`, and (before PostgreSQL 18) `bytea`;
  a 1C base always has them, a hand-made schema might not. The generated
  text stays plain SQL, so the 9.0 portability requirement is kept.
- `MAX` over `varbinary(max)` on SQL Server is documented loosely; task 4.2
  verifies it on the MSSQL demo before the change is archived, and a
  value-storage constant is excluded from the table if it fails.
- `ВЫБРАТЬ *` on a base with hundreds of constants reads hundreds of
  single-row tables in one `UNION ALL`; this is linear work without join
  planning, which is why aggregation was chosen.
