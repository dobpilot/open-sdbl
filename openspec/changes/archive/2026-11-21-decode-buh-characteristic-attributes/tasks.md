- [x] Add native 28-member red regression and exact XML-derived qualified expectations.
- [x] Recognize measured homogeneous wrappers using existing completeness validation.
- [x] Verify malformed counts, missing/duplicate identities, unknown child types and modern wrappers.
- [x] Update release versions and run workspace fmt, Clippy, tests, rustdoc and strict OpenSpec.

Validation: native regression was red before implementation; five focused controls, formatting, workspace Clippy, rustdoc and strict all OpenSpec validation pass. Unfiltered workspace runs retain the independently pending UUID SQL corpus mismatch; its expectations are unchanged. Sandbox-only socket permission failures are recorded separately from the complete local-socket rerun. This change remains unarchived and uncommitted pending that independent review.
