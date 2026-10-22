# Tasks

- [x] Retain native evidence and add red regression.
- [x] Add local strict recognition and atomic shared parsing.
- [x] Verify exact types/qualifiers and malformed controls.
- [x] Freeze for independent review and check modern projection.
- [x] Run fmt, all-target Clippy, complete host tests, warning-free doc and strict validation.
- [x] Return an independently applicable patch without release or archive.

## Validation outcome

Final formatting, all-target Clippy, focused tests, warning-free rustdoc and strict validation pass. The complete serial host workspace run reports 1035 passed, 2 failed, 13 ignored and no filtering across 84 result blocks. The unchanged prior section-absence assertion still awaits explicit approval; the inherited UUID corpus failure has the same 22-query report as the base. Existing test files remain unchanged.

The first workspace run had two additional E0460 doctest failures because an equivalent Clippy correction rebuilt the root rlib while integration tests were still executing. That log is retained separately; the complete final run was repeated without concurrent builds and both doctests pass. The initial comparison-to-empty Clippy diagnostic and successful final rerun are retained separately.

The native 752-catalog comparison changes 13 measured legacy rows, restoring 101 own attributes. The other 739 rows and all modern56/57 projection hashes remain unchanged; all changed own memberships/orders/names match XML. Whole-batch checks cover membership and declaration presence, while exact qualifiers are verified for the reached 16-attribute fixture. Version remains 0.9.20; no release, archive or existing-test update is included.
