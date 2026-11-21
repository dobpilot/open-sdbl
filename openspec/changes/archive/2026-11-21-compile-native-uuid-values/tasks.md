## Tasks
- [x] Add typed UUID parameters and immutable snapshot classification.
- [x] Verify projections, equality, subqueries, UNION ALL, binary preservation and unsupported providers.
- [x] Execute compiled SQL against the own native UUID fixture.
- [x] Connect the BSL UUID parameter through the existing adapter.
- [x] Replay original demo and UNF and record terminal results.
- [x] Run workspace formatting, Clippy, tests, rustdoc and strict validation.

Remaining gate: update the separately proposed UUID corpus SQL expectations only after user approval, then run the unfiltered workspace suite. Current remaining suite: 987 passed, 0 failed, 13 ignored, one corpus test filtered; formatting, Clippy, rustdoc and strict validation pass.

Authorized integration verification on 2026-10-16: both separately proposed existing-test patches are applied after explicit user approval. Unfiltered workspace tests pass:1065passed,0failed,13ignored. Formatting, all-target Clippy, warning-free rustdoc and strict OpenSpec validation pass. Evidence is retained by the consuming workspace at `open-bsl/target/diagnostics/approved-sdk-integration-gates-1016/`. Earlier pending statements above describe historical runs.
