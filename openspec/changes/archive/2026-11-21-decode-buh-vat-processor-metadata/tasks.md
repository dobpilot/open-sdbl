# Tasks

- [x] Retain paired native CF/XML fixture and reproduce missing metadata.
- [x] Decode measured owner, own fields and section fields with complete-or-unknown checks.
- [x] Verify malformed, truncated, duplicate and unsupported headers remain unknown.
- [x] Set three package versions and lockfile to 0.9.18.
- [ ] Run fmt, Clippy, unfiltered workspace tests, rustdoc and strict validation.

## Verification outcome

Native regression: four new tests pass, including exact ordered membership/types and malformed variants. Formatting, all-target Clippy, rustdoc with warnings denied and strict OpenSpec validation pass. The unfiltered workspace run outside the socket-restricted sandbox reports 1008 passed, 1 failed, 13 ignored and zero filtered tests. Its only failure is the inherited query_corpus UUID SQL expectation mismatch (22 demo outputs); the separately pending expectation patch was not applied. The earlier sandbox run additionally denied eight existing local socket tests; those pass in the unfiltered repeat. Final gate remains incomplete, so this change is not archived. Logs: open-bsl/target/diagnostics/open-sdbl-0918-agent/.
