## Implementation
- [x] Retain paired native report CF/XML resources and original UNF report acquisition.
- [x] Add report descriptor fields and measured complete projection with negative tests.
- [x] Verify empty report and extension projection completeness.
- [x] Run formatting, Clippy, workspace tests, rustdoc and strict validation.
- [ ] Assign user-approved version and archive after green checks.

## Verification evidence

- Native report fixture: 8 decoder checks, including native checked empty report, original UNF report, ordered XML GUIDs, qualifiers and uncertainty.
- Selected-acquisition retention: passed without retaining source bytes.
- Formatting, workspace Clippy with denied warnings, rustdoc with denied warnings and all strict OpenSpec validation: passed.
- Workspace tests: root and CLI suites passed; eight DB mock TCP bindings hit sandbox EPERM. Unmodified DB suite repeated outside sandbox: 119 passed, 0 failed, 8 ignored. Workspace doctests passed separately. No test assertions changed.
- Version assignment and archive are intentionally pending user input.
