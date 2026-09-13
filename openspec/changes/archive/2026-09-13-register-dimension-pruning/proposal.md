## Why

A register's virtual table answers one row per combination of the
dimensions the query actually reads: the platform aggregates over the
rest. The compiler grouped by every dimension, so
`ВЫБРАТЬ О.Товар, О.КоличествоОборот ИЗ РегистрНакопления.Продажи.Обороты`
returned one row per товар and клиент rather than one row per товар, with
turnovers split across rows. Measured on the probe base, which gained an
accumulation register and a posting document for this.

The same queries also fail to parse when written the way the platform
allows, without an argument list: `… .Обороты КАК О`.

## What Changes

- `Остатки` and `Обороты` SHALL sum their resources over the dimensions
  the statement never resolves, matching the platform. A statement that
  reads no dimension SHALL get one row.
- A dimension used only inside the virtual table's own condition SHALL
  NOT add a grouping level, as on the platform.
- Every virtual table SHALL be accepted without its argument list.

## Capabilities

### Modified Capabilities

- `query-repl`: register virtual tables group by the dimensions in use.

## Impact

- `src/query/core/parser.rs`, `codegen/virtual_tables.rs`,
  `codegen/context.rs`, `codegen/select.rs`, `codegen/sources.rs`;
  `docs/query-language-support.md`.
