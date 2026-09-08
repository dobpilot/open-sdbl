## Why

The compiler wraps every projected column, scalar, and aggregate in a textual
cast (`::text`, `CONVERT(nvarchar(max), …)`, `CONVERT(varchar(max), …, 1)`).
That made the CLI trivially portable but hides the real value types from every
other consumer: a `CompiledQuery` only lists labels, references arrive as
dialect-specific hex text, numbers lose their declared precision, and an
external connector cannot map the result to native SQL types without parsing
generated SQL.

## What Changes

- **BREAKING** Generated SQL no longer converts projected values to text.
  Physical columns, scalar expressions, and aggregates are emitted in their
  native database types. The only remaining conversion is the MSSQL
  `_YearOffset` correction for date columns and a `::text` cast for the
  PostgreSQL 1C extension types `mchar`/`mvarchar`, whose binary wire format
  is undocumented.
- **BREAKING** `CompiledQuery::columns` becomes `Vec<CompiledColumn>`, where
  each entry carries the emitted label and a structured `ColumnKind`
  (reference with target object IDs, binary, string, number with precision and
  scale, boolean, date-time, UUID, null, unknown). `QueryableColumn` exposes
  the same kind for every physical member.
- **BREAKING** A reference is always one output column: a field without an
  `RTRef` member projects its 16-byte `RRRef`; a runtime-typed reference
  projects the 20-byte concatenation `RTRef ‖ RRRef`. Deferred presentation
  payloads and batch presentation lookups use the same binary form instead of
  hex text.
- UNION branches whose output kinds differ in the same column position fail
  with a typed diagnostic before execution; `NULL` literals and unknown catalog
  types remain compatible with every kind.
- The CLI reads native values through typed cells on both backends, keeps
  deferred presentation resolution on raw bytes, and renders cells with one
  backend-independent formatting policy.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-compilation`: typed output column metadata and native-typed
  projections.
- `query-repl`: native aggregates, source-free scalars, binary reference
  presentation payloads, and client-side cell rendering.

## Impact

- Public API of `open-sdbl`: `CompiledQuery::columns`, `QueryableColumn`,
  new `CompiledColumn` and `ColumnKind` types, new
  `MetadataSnapshot::object_id_by_physical_table` lookup.
- Every golden SQL fixture that pinned a textual cast.
- CLI output: binary cells print as `0x…` on both backends, booleans as
  `true`/`false`, dates as `YYYY-MM-DD HH:MM:SS`.
- No new production dependencies; PostgreSQL binary decoding for `numeric`,
  `timestamp`, and `uuid` is implemented in the CLI crate.
