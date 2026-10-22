## Contract
- [x] Inspect supported settings storage and validate strict delta before code.
## Implementation
- [x] Share existing fixed storage identities and retain supported physical facts with separator mappings.
- [x] Compare selected/cache/whole SQL and scope validation using native fixtures; preserve unknown table exclusion and strict missing descriptors.
- [x] Bump package versions to 0.9.35.
## Verification
- [x] Run focused and workspace tests, formatting, Clippy, warning-free docs and strict validation.
- [x] Archive the change and deliver early implementation plus final delta.

Verification: native core parity 1/1 and strict/cache integration 2/2; workspace 1147 passed, 13 ignored, 0 failed including doctests; fmt, all-target Clippy with warnings denied, warning-free docs and strict validation passed. Existing test expectations were not changed.
