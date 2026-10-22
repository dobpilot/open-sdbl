# Tasks

- [x] Retain measured native CF/XML evidence and reproduce missing section metadata with a new red regression.
- [x] Extract shared staged typed-row parsing and add atomic legacy section publication without changing the modern path.
- [x] Verify six section identities/order, 68 fields, all 201 qualified type rows and malformed cases preserving the seven own attributes.
- [x] Compare all 455 native document section projections and report every changed row.
- [x] Run fmt, all-target Clippy, complete unfiltered host workspace tests, warning-free rustdoc and strict validation.
- [x] Return a minimal independently applicable patch against the frozen own-attribute review without releasing or archiving.

## Validation outcome

Formatting, all-target Clippy, warning-free documentation and strict OpenSpec validation pass. The complete unfiltered host workspace run reports 1033 passed, 2 failed and 13 ignored across 84 result blocks. One failure is the inherited UUID query-corpus baseline. The other is the unchanged prior own-attribute test asserting that tabular sections are unknown; the native fixture now proves six sections. Its separate one-line update proposal remains unapplied pending explicit approval. No existing test file was changed.

The 455-document comparison changes four legacy rows (10 sections and 103 fields); all remaining 451 rows, modern class-40 rows and existing own-attribute metrics are unchanged. The batch validates membership/order/name and declaration presence; exact qualifier equality is independently checked for all 201 type rows of the reached document. The isolated review remains version 0.9.20 and is neither released nor archived.

Authorized integration verification on 2026-10-16: the user approved both existing-test proposals. Both are now applied. Unfiltered workspace tests pass:1065passed,0failed,13ignored; formatting, all-target Clippy, warning-free rustdoc and strict OpenSpec validation pass. The earlier validation outcome is historical. No release or archive was performed by this integration step.
