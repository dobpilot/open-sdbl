## 1. Extension restructure parsing

- [x] 1.1 Add `parse_extension_restructure` in `src/metadata/extension.rs`
  that decodes `_ExtensionsRestruct._restructData` blobs (UTF-8-BOM and
  raw DEFLATE both handled by the existing decoder) into records
  `{ guid, number, name, sql_type, reference_target }`. Unit test on
  `restruct_reference14574.txt` asserts the three СтавкиНДС mappings
  (Fld16536 STRING, Fld16537 NUMERIC, Fld16538 REF).

## 2. Resolution

- [x] 2.1 Add `extension_metadata_from_restructure` building an
  `ExtensionMetadata` (extension DBNames `Fld` entries + name
  descriptors) that `resolve_metadata_with_extensions` merges into the
  owning object with `extension_origin` set; owner linkage by physical
  `…x1` column. Base-only resolution stays byte-identical (tested).
- [x] 2.2 `parse_extension_restructure` returns `ExtensionRestructure`
  with a separate `anomalies` list; a `Fld`-named record that fails to
  decode becomes `ResolutionFinding::MalformedExtensionRestructure`
  through the resolver. The existing `ExtensionFieldNumberConflict` for
  colliding numbers is retained.

## 3. Compilation

- [x] 3.1 Extension-union projection includes the merged `_fldN` columns:
  `merged_extension_projection` now admits an `…x1` column when it
  matches a registered extension field, and both custom-name resolvers
  address extension fields by their globally unique number. Explicit
  selection and `SELECT *` return extension data.
- [x] 3.2 Reference-typed extension attributes carry their
  `reference_target` (via `MetadataField.reference_target`, sourced from
  the restructure) into `merged_extension_projection`, so a dereference
  through an extension reference compiles. Verified end to end.

## 4. CLI integration

- [x] 4.1 Add SELECT-only `EXTENSION_RESTRUCTURE` queries for PostgreSQL
  and MSSQL (`_ExtensionsRestruct`), included in `all()` and covered by
  the SELECT-only test.
- [x] 4.2 The unified pipeline reads `_ExtensionsRestruct` (new trait
  method on both providers), decodes each blob via
  `parse_extension_restructure` + `extension_metadata_from_restructure`
  in `decode_extension_restructures`, and merges the result into
  `resolve_metadata_with_extensions`. A blob that fails to decode is
  skipped with a warning. Verified live against the `demo` base:
  `SELECT Расш1_Реквизит1, Расш1_Реквизит2 FROM Справочник.СтавкиНДС`
  returns extension data.

## 5. Verification

- [x] 5.1 Conformance test resolving the СтавкиНДС fixture end to end:
  `Расш1_Реквизит1..3` queryable, compiled SQL selects `_fld16536` on
  both dialects, base-only resolution cannot see the attributes.
- [x] 5.2 fmt, clippy `-D warnings`, workspace tests, rustdoc, and
  `cargo audit` pass; `openspec validate resolve-extension-attributes
  --strict` passes.
- [x] 5.3 README documents extension-attribute resolution.
