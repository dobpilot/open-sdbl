## Context

Enumeration values are descriptors nested in the owning enumeration's
bare-GUID Config resource. Catalog predefined values are serialized rows in
`<catalog-guid>.1c`. Both store canonical textual GUIDs, while PostgreSQL and
MSSQL 1C tables store their 16 bytes in UUID field order `d + e + c + b + a`.

An enumeration row uses those bytes directly as `_IDRRef`. A catalog row has a
database-specific `_IDRRef`; its stable metadata GUID is stored in
`_PredefinedID`. Therefore a catalog constant cannot safely be fabricated from
the metadata GUID.

## Goals / Non-Goals

**Goals:**

- resolve exact metadata names without inspecting business data;
- preserve read-only, dependency-free core architecture;
- generate equivalent PostgreSQL and MSSQL expressions;
- keep failures positional and deterministic.

**Non-Goals:**

- support every 1C value kind in the first increment;
- cache database-specific catalog `_IDRRef` values in metadata;
- infer names from code or description columns.

## Decisions

### Decode only the authoritative `.1c` suffix

Metadata acquisition selects bare GUID resources and `<guid>.1c`. Other
suffixes contain unrelated binary or presentation payloads. The `.1c` parser
accepts only rows with the verified seven-column predefined-value signature.

### Build one resolved value index

Resolution projects enumeration child descriptors and catalog `.1c` rows into
one owner/name index. Ambiguous normalized names are retained as ambiguity,
never resolved by source order.

### Preserve distinct physical semantics

Enumeration `VALUE` emits the GUID in 1C byte order. Catalog `VALUE` emits a
scalar subquery over the resolved live table, comparing `_PredefinedID` with
the stable GUID and returning `_IDRRef`. Both expressions remain usable in
projections and predicates; the existing cross-source-field-only JOIN contract
is unchanged.

### Strict syntax and supported kinds

The function accepts exactly three path components. Only catalog and
enumeration kinds are accepted initially. The English alias `VALUE` follows
the bilingual conventions of the existing compiler.

## Risks / Trade-offs

- Catalog constants add a scalar lookup to generated SQL. The platform's
  `_PredefinedID` index bounds that lookup and avoids loading catalog data into
  the application.
- A database inconsistent with its Config metadata can make the scalar lookup
  return NULL. The compiler validates the resource, object, value, live table,
  and required columns, but intentionally performs no core-library I/O.
