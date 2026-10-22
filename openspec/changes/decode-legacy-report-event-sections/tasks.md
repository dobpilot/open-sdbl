# Tasks

- [x] Retain paired native evidence and reproduce missing section membership with new red tests.
- [x] Parameterize the existing staged decoder and publish complete catalog sections atomically.
- [x] Verify ordered identities/types/qualifiers and malformed late-section/field/collision cases preserving own metadata.
- [x] Freeze implementation and report the exact diff for independent integration review.
- [x] Run sequential fmt, all-target Clippy, full host workspace tests, warning-free docs and strict validation; retain inherited failures.
- [x] Return a portable patch and evidence receipt without release, commit or archive.

## Validation outcome

Formatting, all-target Clippy, warning-free rustdoc and strict validation pass. Focused tests pass 3/3, including both retained native pairs and 21 malformed/collision controls. The complete sequential host workspace run reports 1038 passed, 2 failed, 13 ignored and 0 filtered across 84 result blocks. Both failures are inherited: the unchanged old document section-absence assertion (awaiting explicit approval) and the identical 22-query UUID corpus baseline. Existing tests are not edited.

Independent public verification confirms the reached owner seven own attributes, one section, four fields and all 12 exact type rows. The 752-catalog comparison changes 12 legacy rows, restores 3 parts/7 fields, preserves 740 rows including 727 modern non53 projections, and matches changed memberships/orders/names/type presence to XML. Qualifier completeness is claimed only for the reached owner. The complete 455-document regression is byte-identical.

The source is frozen. Version remains 0.9.20; no commit, release or archive is performed while inherited mandatory gates remain red.
