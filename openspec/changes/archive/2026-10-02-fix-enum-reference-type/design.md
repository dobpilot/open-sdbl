# Design

Compare original UNF Config resources against native Designer XML exported with 1C 8.3.27.2342. Class 20 stores EnumRef TypeId at position 1, its ValueId at position 2, EnumManager TypeId at position 3. Select the measured enum position only; retain the existing other-class projection. No BSL execution behavior or document numbering changes. Unknown class layouts are not generalized from this finding.

## Evidence and limits

`tests/fixtures/runtime-metadata-096/enum-native.tsv` records object UUID, name, native EnumRef TypeId and native EnumManager TypeId for all 426 UNF enumerations present in `tests/fixtures/unf/config.pack`. Native XML is the oracle; interpreter output is not used to create expected identities. Every class-20 prefix was independently checked against both native generated types before saving the oracle. The new regression failed before the fix and passes afterward. ВариантыВажности (`19430845-e6b2-4a2b-944a-75142d4914c0`) now exposes `981c7e3c-694b-4328-8937-217bb303823c`, matching the already acquired filling value of document Событие. No database writes, new external dependencies, or other object layouts were introduced. Live SQL Server and Windows are not newly validated.

## Validation

Formatting, workspace Clippy with warnings denied, workspace tests (819 passed, 13 ignored, zero failed), rustdoc with warnings denied, and all strict OpenSpec validation passed. The consumer independently confirmed its native enumeration filling regression and all 61 bsl-config-db tests pass. Existing regression expectations were not changed.
