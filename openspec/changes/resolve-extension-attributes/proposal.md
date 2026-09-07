## Why

`support-extension-service-tables` left task 1.2 open: extension-added
attributes were not readable because their declarations live outside the
base metadata, and the mapping had not been captured from a live base.
A query against a real extended catalog exposes the gap concretely:

```
select Расш1_Реквизит1 from Справочник.СтавкиНДС;
error: field "Расш1_Реквизит1" was not found in source
```

`select *` returns only the 13 base columns unioned across
`_reference14574` and `_reference14574x1`; the three extension
attributes that exist only in the `…x1` table are invisible, and a
reference-typed extension attribute would lose its data entirely.

Live-base inventory (PostgreSQL `demo`, extension "Расширение1") found
the authoritative mapping in `_ExtensionsRestruct._restructData`, a
brace-serialized resource the existing decoder already reads. Each
extension attribute appears as one record:

```
{7d8d7de3-…,"Fld16536","STRING(10) VARYING","Расш1_Реквизит1",…}
{180244b5-…,"Fld16537","NUMERIC(10)","Расш1_Реквизит2",…}
{3c3de8c8-…,"Fld16538","REF(Reference16531)","Расш1_Реквизит3",…}
```

This record set is self-sufficient: it yields the attribute's logical
name, physical `Fld` number (hence the `_fldNNNNN` column in the `…x1`
table), type, and reference target. No new low-level decoder is needed —
both `ConfigCAS` blobs and `_restructData` are the familiar
raw-DEFLATE / UTF-8-BOM brace format.

## What Changes

- Parse `_ExtensionsRestruct._restructData` into a typed extension-field
  restructure map (attribute GUID → physical `Fld` name → type → logical
  name → owner object).
- Feed that map through the existing `resolve_metadata_with_extensions`
  entry point so extension attributes merge into their owning object's
  queryable fields, mapped to the physical `_fldNNNNN` column and marked
  with their extension origin — completing the mechanism that was
  stubbed for task 1.2.
- Add SELECT-only `_ExtensionsRestruct` acquisition queries for both
  providers and wire them into the unified CLI pipeline.
- Ensure the `…x1` extension columns for the merged attributes appear in
  the compiled projection so `SELECT *` and explicit selection return
  extension data on both dialects.

## Capabilities

### Modified Capabilities

- `onec-metadata`: resolve extension-added attributes from the
  extension restructure resource, not only from caller-synthesized
  input.

## Impact

- Library gains extension-restructure parsing; the CLI fetches one more
  read-only resource. No new dependencies.
- Fixtures captured verbatim from the live `demo` base
  (`tests/fixtures/service_tables/extension_attrs/`) drive the tests.
- Closes the outstanding task 1.2 from the archived
  `support-extension-service-tables` change.
