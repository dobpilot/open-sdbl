## Why

1C queries read several constants at once through the `Константы` table
(`ВЫБРАТЬ Константы.ОсновнаяВалюта, Константы.ИспользоватьСклады ИЗ
Константы`), and reports and BSP code depend on it. The compiler accepts
only `Константа.Имя`, one constant per source, so such queries fail with an
unknown-object diagnostic and callers have to rewrite them as joins of
single-constant sources. Physically every constant lives in its own
`_Const<N>` table with one row per data area, so a naive rewrite joins as
many tables as there are constants referenced, up to hundreds for
`ВЫБРАТЬ *`.

## What Changes

- The parser and resolver SHALL accept `Константы`/`Constants` as a
  source (with or without alias) whose fields are the names of every
  constant with a live `_Const<N>` table. The table has no standard fields;
  `*` projects every constant; reference-typed constants dereference
  through the ordinary join machinery.
- The source SHALL render as a derived table that reads only the constants
  the statement uses: a `UNION ALL` of one `SELECT` per constant, each
  projecting that constant's physical columns and `NULL` for the others,
  aggregated with `MAX` and no `GROUP BY`, so the result is exactly one
  row even when no constant has ever been written (all `NULL`).
- Separator predicates from `data-separator-predicates` apply inside
  every branch. A branch whose separator is disabled for the statement
  SHALL be an `UnsupportedFeature` diagnostic naming the constant, because
  one row per area cannot be expressed without a grouping key.
- `Константа.Имя` keeps working unchanged. The pre-8.2.14 single `_Consts`
  table is not supported.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-compilation`: `Константы` source, its rendering, and its
  diagnostics.

## Impact

- `src/query/core/resolve.rs` (source kind and field catalog for the
  constants table), `codegen/sources.rs` (relation rendering),
  `codegen/context.rs` (column kinds per constant), `normalize.rs`
  (`RecordKey` recase token).
- CLI: completion and `\d Константы` list the constants table; README and
  `docs/query-language-support.md` (row 158 flips to ✅).
- Depends on `data-separator-predicates` for the separator predicate
  inside branches; can be implemented after it only.
