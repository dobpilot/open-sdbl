## Context

`SqlDialect::column_text`, `scalar_text`, `text`, and `binary_hex_text`
convert every projected value to text so that the CLI can read all cells as
strings. `CompiledQuery` therefore carries only labels. Reference fields with
an `RTRef` discriminator expand into two text columns, and deferred
presentation payloads pack `RTRef:RRRef` as hex text.

Consumers now need native values: the CLI wants to format them itself, and an
external connector needs enough type information to declare result columns
before reading rows.

## Decisions

### Structured column kinds derived from the live catalog

`ColumnKind` is a `#[non_exhaustive]` enum:

| Kind | Payload | Source |
|---|---|---|
| `Reference` | `targets: Vec<ObjectId>`, `runtime_typed: bool` | SchemaStorage `R` targets resolved to object IDs; `runtime_typed` when the field has an `RTRef` member |
| `Binary` | `length: Option<u32>` | `bytea`, `binary(n)`, `varbinary(n)`, `image`, `timestamp`/`rowversion` |
| `String` | `length: Option<u32>` | `text`, `character varying(n)`, `character(n)`, `mchar(n)`, `mvarchar(n)`, `n?varchar(n)`, `n?char(n)`, `n?text` |
| `Number` | `precision: Option<u8>`, `scale: Option<u8>` | `numeric(p,s)`, `decimal(p,s)`, integer types with their decimal capacity and scale 0, floating types with no declared capacity |
| `Boolean` | — | `boolean`, `bit` |
| `DateTime` | — | `timestamp…`, `date`, `datetime`, `datetime2`, `smalldatetime` |
| `Uuid` | — | `uuid`, `uniqueidentifier`, `UUID()` expressions |
| `Null` | — | the `NULL` literal |
| `Unknown` | `data_type: String` | any other catalog type |

The catalog strings come from PostgreSQL `format_type` and the MSSQL catalog
query, both of which include length, precision, and scale where the type
declares them, so kinds are derived without extra database round trips.

Reference targets are resolved through a new snapshot index from canonical
physical table name to object ID. Targets whose table is absent from the
snapshot are skipped rather than failing, because a projection of such a
field is still valid; presentation compilation keeps its stricter checks.

### One column per reference

A pure reference field projects exactly one SQL column. Without an `RTRef`
member the column is the 16-byte `RRRef`. With an `RTRef` member the column
is the binary concatenation `RTRef ‖ RRRef` (PostgreSQL `||`, MSSQL `+`),
which is 20 bytes: a 4-byte big-endian table number followed by the 16-byte
reference. `NULL` propagates naturally. The other members of a compound field
(`_TYPE`, `_N`, `_S`, `_L`, `_T`) remain separate columns with their own
kinds and suffixed labels.

Deferred presentation payloads reuse the same 20-byte column. Batch
presentation lookups return the raw `_IDRRef` as the `__reference` column.

### Native projections with two documented exceptions

`column_projection` emits the physical column as-is except for:

- MSSQL date columns with a non-zero `_YearOffset`, which are wrapped in
  `DATEADD(year, -offset, …)` so the logical date is returned; this mirrors
  the `+offset` applied to date literals in predicates.
- PostgreSQL `mchar`/`mvarchar` columns, which are cast to `text`. The 1C
  extension's binary send format is undocumented, so the text protocol is the
  only portable representation.

`ПРЕДСТАВЛЕНИЕ`/`PRESENTATION` keeps converting scalar arguments and template
fields to text because the function's result is a string by definition.

### Expression kinds

Scalar projections derive their kind from the expression: literals map to
`Number`/`String`/`Boolean`/`Null`/`Binary`, date constructors to `DateTime`,
`ЗНАЧЕНИЕ` to `Reference`, fields to their column kind, comparisons and
logical operators to `Boolean`, arithmetic to `Number` without declared
capacity. `COUNT` and `SUM` are `Number`; `MIN`/`MAX` inherit the argument
column kind.

### UNION compatibility

Branches are compared position by position on the enum variant only;
`Null` and `Unknown` match everything. A mismatch fails with
`UnsupportedFeature` positioned at the `ОБЪЕДИНИТЬ`/`UNION` token, which is
clearer than the database error that native types would otherwise produce.

### Client-side cells

The CLI introduces `Cell` (`Null`, `Text`, `Bytes`, `Number`, `Bool`,
`DateTime`, `Uuid`). PostgreSQL rows are decoded through a local `FromSql`
implementation over the binary protocol; `numeric`, `timestamp`, `date`, and
`uuid` decoders are implemented in the CLI crate so that no dependency is
added. MSSQL rows are decoded from `ColumnData`. Rendering is one policy for
both backends: `0x` plus upper-case hex for bytes, `true`/`false`,
`YYYY-MM-DD HH:MM:SS` with fractional seconds dropped, numbers with their
declared scale, canonical lower-case UUIDs.

## Risks / Trade-offs

- Any consumer that relied on all-text rows must adopt `ColumnKind`.
- `mchar`/`mvarchar` remain text; a future change can add binary decoding once
  the extension format is verified.
- Fractional seconds are not displayed by the CLI; 1C stores none.
