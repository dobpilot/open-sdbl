## 1. Workspace defaults

- [x] 1.1 Select `crates/open-sdbl-cli` in `workspace.default-members`.
- [x] 1.2 Document that the ordinary root release build produces the CLI.

## 2. Verification

- [x] 2.1 Verify default, explicit library-only, and whole-workspace release
  builds using clean temporary target directories.
- [x] 2.2 Run workspace formatting, Clippy with warnings denied, tests, rustdoc
  with warnings denied, and strict OpenSpec validation.
