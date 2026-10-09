## 1. Evidence and contract
- [x] Inspect current specifications and common compiler path.
- [x] Retain native controls for both modes and usage/value decision order.
- [x] Validate the specification strictly before code.

## 2. Implementation
- [x] Add failing regression for disabled Independent and lazy flag requests.
- [x] Implement mode-aware states in the existing common compiler.
- [x] Verify unused tables, preparation, constants, dereference and virtual tables on both backends.
- [x] Update workspace version to 0.9.13.

## 3. Verification
- [x] Run workspace fmt, Clippy, tests and rustdoc.
- [x] Validate all OpenSpec changes strictly.
- [x] Commit implementation and archive separately after successful checks.
