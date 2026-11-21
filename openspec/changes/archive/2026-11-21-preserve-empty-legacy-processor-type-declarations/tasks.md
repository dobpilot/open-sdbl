# Tasks

- [x] Retain the native CF/XML pair and a red exact-fixture regression.
- [x] Preserve known empty declarations without weakening malformed-profile checks.
- [x] Verify all 43 native names, ordered identities, types and qualifiers, plus malformed negative cases.
- [x] Run formatting, all-target Clippy, unfiltered workspace tests, warning-free rustdoc and strict validation.
- [x] Return a minimal review patch against the complete SDK 0.9.20 stack without releasing or archiving.

## Verification outcome

The exact native regression failed before implementation and passes afterwards: two tests, including nine malformed cases. All 43 ordered identities, names, declared types and qualifiers match the retained native pair; four declarations are known empty and three retain ValueTable identity. The same pair verifies two nonempty textual comments. Runtime changes are limited to the measured legacy fallback accepting textual comments and successfully parsed empty declarations.

Formatting, all-target Clippy with warnings denied, warning-free rustdoc and strict OpenSpec validation (23 items) pass. The initial unfiltered sandbox run reports 1016 passed, 9 failed, 13 ignored and zero filtered: one inherited UUID SQL corpus mismatch and eight existing localhost socket tests denied by the sandbox. The complete unfiltered host repeat reports 1024 passed, 1 failed, 13 ignored and zero filtered tests. Its only failure is the inherited query_corpus::compiles_the_demo_corpus_as_recorded; all eight socket tests pass outside the sandbox. The final green gate therefore remains blocked by the separately pending corpus expectation approval. No existing test expectations, versions or lockfiles were changed. No release, merge or archive is performed.
