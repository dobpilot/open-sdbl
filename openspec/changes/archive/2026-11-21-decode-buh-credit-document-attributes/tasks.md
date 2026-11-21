- [x] Validate the OpenSpec delta before implementation.
- [x] Reproduce missing native23 membership against unchanged SDK19.
- [x] Preserve exact23 names, order, reference identities and qualified types from native XML.
- [x] Reject malformed wrappers, identities, unsupported declarations, missing and duplicate descriptors.
- [x] Preserve modern3/9 identities with both measured5/5 and6/7 wrappers.
- [x] Verify native Object/Ref TypeId/ValueId and unchanged reference identity projection.
- [x] Compare all455native document owners; restore20/188 and retain384/8496 modern baseline.
- [x] Complete workspace verification and record the known baseline corpus failure.
- [ ] Archive only after an entirely green workspace run; pending UUID corpus expectations are not authorized.

## Verification

The full unfiltered workspace run with eight new native tests reports1021passed,1failed,13ignored,0filtered. The only failure remains tests/query_corpus.rs::compiles_the_demo_corpus_as_recorded, the unapproved pre-existing UUID SQL expectations. No existing tests or expectations were changed.

After that complete run, a ninth focused assertion was added at review request for the same native document's existing numbering/empty memberships. The resulting focused suite passes9/9; no runtime code changed. Formatting, all-target Clippy, rustdoc with warnings denied and strict OpenSpec were repeated on that final test revision. The full1021/1/13 result predates only that ninth assertion; it is not represented as a full run containing nine new tests.

The change remains active and uncommitted because the workspace is not entirely green. No release commit, archive or main merge was performed.
