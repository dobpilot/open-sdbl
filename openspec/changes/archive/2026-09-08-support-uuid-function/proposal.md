## Why

1C references are stored as 16 bytes in the platform's own field order, so a
reference projected by `open-sdbl` cannot be compared with the UUID that 1C
shows in `УникальныйИдентификатор(Ссылка)` without a client-side byte
permutation. Users need the canonical UUID directly from the query.

## What Changes

- Recognize the bilingual function `УНИКАЛЬНЫЙИДЕНТИФИКАТОР`/`UUID` as a
  keyword that remains usable as a contextual identifier.
- Compile `UUID(<reference field>)` to native `uuid` on PostgreSQL and
  `uniqueidentifier` on MSSQL with pure SQL byte permutation, no server
  extension. `NULL` references stay `NULL`.
- Accept pure reference fields, dereferenced reference paths, and compound
  fields through their `RRRef` member; reject non-reference fields, literals,
  `ЗНАЧЕНИЕ`, and source-free branches with typed diagnostics.
- Report the result as `ColumnKind::Uuid`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `sdbl-lexer`: one more bilingual keyword.
- `query-repl`: compile reference UUID expressions.

## Impact

- Lexer keyword table grows to 47 entries.
- No new dependencies; no I/O.
