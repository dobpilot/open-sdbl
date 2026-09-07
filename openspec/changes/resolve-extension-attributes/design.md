# Design

## Guiding constraints

- Core stays dependency-free and I/O-free: `_restructData` arrives as a
  caller-provided blob, decoded by the existing raw-DEFLATE + brace
  parser. No new decoder.
- Every parsing rule cites a real fixture under
  `tests/fixtures/service_tables/extension_attrs/`.
- Existing golden SQL is frozen; extension attributes only add columns.

## 1. Extension restructure format (captured from live `demo`)

`_ExtensionsRestruct._restructData` rows: some carry a UTF-8 BOM, some
are raw DEFLATE — both already handled. The relevant record shape,
verbatim from `restruct_reference14574.txt`:

```
{<fieldGuid>,"Fld<N>","<TYPE>","<LogicalName>",flags…}
```

grouped under a parent block keyed by the metadata-type GUID and the
extended object. Types observed: `STRING(n) VARYING`, `NUMERIC(p)`,
`REF(Reference<N>)`. The physical column is `_fld<N>` (with the `rref`
suffix for `REF`, matching the live `…x1` catalog:
`_fld16538rref`). The owning object is identified by GUID and cross-checked
against the ConfigCAS object blob (`ext_object_stavkinds.txt`), where
"СтавкиНДС" (GUID `e1945025-…`) contains "Расш1_Реквизит1".

## 2. Parsing

Add `parse_extension_restructure(blobs) -> ExtensionRestructure` in
`src/metadata/`. It walks the brace tree, collecting per-object the list
of `{ field_guid, physical_field: "FldN", type, logical_name,
reference_target }`. Malformed records are skipped and reported (a
finding, consistent with the totality guarantees), never panicked on.

## 3. Resolution

`ExtensionMetadata` gains a `restructure: ExtensionRestructure` field (or
a new sibling input). `resolve_metadata_with_extensions` uses it to
build extension `QueryableField`s directly: logical name from the
restructure, physical column `_fldN` from the same record, type from the
declared SQL type, `extension_origin` set. This replaces the synthetic
path exercised in the archived change's test with the real mapping.
Owner linkage is by GUID (DBNames-authoritative rule); attribute-number
collisions keep the existing `ExtensionFieldNumberConflict` finding.

## 4. Compilation

No new syntax. Once merged, extension attributes are ordinary fields, so
the existing extension-union projection must include the `_fldN`
columns. Confirm the `…x1` union projects the merged columns (the bug in
the report: the projection currently lists only base-declared columns).
Golden SQL on both dialects for `SELECT Расш1_Реквизит1`,
`Расш1_Реквизит3.<attr>` (reference dereference), and `SELECT *`.

## 5. CLI

- `queries.rs`: `EXTENSION_RESTRUCTURE` SELECT-only for both providers
  (`_ExtensionsRestruct`), added to `all()` so the SELECT-only test
  covers them.
- Pipeline reads the restructure alongside the already-fetched
  `ConfigCAS` extension resources and passes both to
  `resolve_metadata_with_extensions`.

## 6. Testing

- Unit: `parse_extension_restructure` on `restruct_reference14574.txt`
  yields the three mappings with correct physical names/types.
- Resolution: extended snapshot exposes `Расш1_Реквизит1..3` as
  queryable fields on object СтавкиНДС mapped to `_fld16536/…/_fld16538rref`.
- Golden SQL both dialects: explicit selection, reference dereference,
  and `SELECT *` include the extension columns.
- Regression: base-only resolution (no restructure) unchanged; a
  genuinely undeclared table still reported.
