# Tasks

- [x] Validate strict before code.
- [x] Retain native Config/XML evidence and acquire the condition reference.
- [x] Add typed condition values and diagnostics.
- [x] Test both backends and affected read paths, preparation and changes.
- [x] Run all workspace gates and strict validation.
- [x] Commit implementation, archive separately.

Verification: native 8.3.27.2342 own CF/XML pairs and 21-line receipt; 10 new focused tests; full workspace 900 passed, 0 failed, 13 ignored, 0 filtered; fmt, Clippy all-targets, rustdoc deny-warnings and strict all (8/8) passed. No SQL expected corpus was modified.
