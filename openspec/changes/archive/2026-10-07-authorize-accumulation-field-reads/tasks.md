## 1. Implementation
- [x] 1.1 Preserve native evidence and verify provenance.
- [x] 1.2 Implement verified accumulation dimension/resource field authorization.
- [x] 1.3 Cover owner refusal, defaults, missing users/roles, unsupported membership and multiple roles/RLS.
- [x] 1.4 Set workspace package versions to 0.9.10.
## 2. Validation
- [x] 2.1 Run formatting, Clippy, workspace tests and rustdoc with warnings denied.
- [x] 2.2 Validate OpenSpec strictly before separate archival.

Evidence: /tmp/open-sdbl-0910-final-{fmt,clippy,tests,doc,openspec,targeted}.log. Workspace: 861 passed, 0 failed, 13 ignored. All 52 native provenance hashes verified. Existing tests remain unchanged; the new independent integration target has three tests, including four negative descriptor modes.
