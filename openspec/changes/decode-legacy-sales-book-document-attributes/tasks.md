# Tasks

- [x] Retain native CF/XML and reproduce the seven missing declarations and membership with a red regression.
- [x] Extend the existing strict document attribute helper with the measured legacy profile and preserve staged publication.
- [x] Verify every native identity, name, type and qualifier and malformed negative cases without editing existing tests.
- [x] Compare the complete 455-document native corpus before and after and report every changed row.
- [x] Run fmt, all-target Clippy, complete unfiltered host workspace tests, warning-free rustdoc and strict validation.
- [x] Return an independently applicable minimal patch against the frozen news review without changing versions.

## Verification outcome

The exact native regression failed against the frozen news review and passes after implementation. Three new tests verify all seven ordered native GUIDs, names, types and qualifiers and seventeen malformed owner/member variants without prefix publication. Nine unchanged existing credit-document tests pass, including modern wrapper and malformed-profile controls. A separate consumer verifier confirmed seven complete declarations and that legacy numbering, registers and tabular sections remain unknown.

The complete 455-document native before/after batch changes exactly five class 38/51 rows, restoring 35 attributes with native GUID order. The other 450 rows remain identical. This does not mean all class 38 profiles are supported: 409 owner memberships are known, 46 remain unknown (32 class 38/51 and 14 class 37/50). All remaining class 38/51 rows have nonempty owner or member identity comments outside the reused helper's current measured constraints; no further support is inferred or implemented.

Formatting, all-target Clippy with warnings denied, warning-free rustdoc and strict OpenSpec validation pass. The complete unfiltered host workspace run reports 1030 passed, 1 failed, 13 ignored and zero filtered tests across 84 summary blocks. Its only failure is the inherited query_corpus::compiles_the_demo_corpus_as_recorded UUID SQL expectation mismatch. Final green remains contingent on that separately pending approval. No old tests, versions or lockfiles changed; no release, merge, commit or archive is performed.
