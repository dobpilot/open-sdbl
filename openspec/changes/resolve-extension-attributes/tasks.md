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
- [ ] 2.2 Emit a typed finding for malformed restructure records (parser
  currently skips them silently). Keep the existing
  `ExtensionFieldNumberConflict` for colliding numbers.

## 3. Compilation

- [x] 3.1 Extension-union projection includes the merged `_fldN` columns:
  `merged_extension_projection` now admits an `…x1` column when it
  matches a registered extension field, and both custom-name resolvers
  address extension fields by their globally unique number. Explicit
  selection and `SELECT *` return extension data.
- [ ] 3.2 Reference-typed extension attributes (`Расш1_Реквизит3` →
  `REF(Reference16531)`) need their `reference_target` carried into the
  merged schema so dereference through an extension reference compiles.
  Scalar attributes are covered by golden SQL on both dialects.

## 4. CLI integration

- [x] 4.1 Add SELECT-only `EXTENSION_RESTRUCTURE` queries for PostgreSQL
  and MSSQL (`_ExtensionsRestruct`), included in `all()` and covered by
  the SELECT-only test.
- [ ] 4.2 Pipeline decodes the restructure (via
  `parse_extension_restructure` + `extension_metadata_from_restructure`)
  and passes it to `resolve_metadata_with_extensions`, replacing the
  empty `read_extensions` stub.

## 5. Verification

- [x] 5.1 Conformance test resolving the СтавкиНДС fixture end to end:
  `Расш1_Реквизит1..3` queryable, compiled SQL selects `_fld16536` on
  both dialects, base-only resolution cannot see the attributes.
- [x] 5.2 fmt, clippy `-D warnings`, workspace tests, rustdoc, and
  `cargo audit` pass; `openspec validate resolve-extension-attributes
  --strict` passes.
- [x] 5.3 README documents extension-attribute resolution.
