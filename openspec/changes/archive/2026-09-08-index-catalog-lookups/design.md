## Context

`CompilationCatalog::fields` scanned `live_tables()` twice and
`schema().tables` once per extension pass, charging each scan to the work
budget. `merged_extension_projection` additionally scanned `fields()` per
column. The charges bounded CPU but made the budget a function of base size.

## Decisions

- The snapshot already indexes objects; the same `MetadataIndex` now holds
  lower-case maps for live tables, extension variants grouped by canonical
  base name (sorted by name for deterministic `UNION ALL` order), schema
  tables, extension-field logical names, and extension reference targets by
  field number.
- The budget charges one unit per extension variant plus one per merged
  column, which is the real cost of projecting a source after indexing.
- `SchemaStorage::table` remains for callers holding a bare schema; the
  compiler uses the indexed `MetadataSnapshot::schema_table`.

## Risks / Trade-offs

- Index construction adds a few hash inserts per table and field at
  resolution time, negligible against Config decoding.
