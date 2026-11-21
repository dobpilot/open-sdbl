## Tasks
- [x] Retain native BUH pair and existing independent false/true evidence.
- [x] Reproduce the public old-profile failure with a new external test.
- [x] Add strict local staged projection and malformed/unknown controls.
- [x] Verify focused tests and independent public/corpus checks.
- [x] Run serial fmt, Clippy, full host tests, rustdoc and strict validation, retaining inherited failures.
- [x] Produce portable patch, exact apply-tree and source receipts.

Validation: four focused tests passed, including six native text-comment pairs, independent existing modern false/true controls and 32 malformed controls. Full host workspace: 1063 passed, 2 inherited failures, 13 ignored, 0 filtered. Fmt, all-target Clippy, rustdoc and strict OpenSpec passed (36 items). No old tests were edited; no archive or release is authorized. Independent corpus: 121 newly recognized old profiles and 362 previous known options match native XML, 64 other profiles remain unknown, 1980 non-options are unchanged, and all 2527 normalized full Debug SHA256 hashes are unchanged.
