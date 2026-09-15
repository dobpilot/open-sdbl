## 1. Extraction

- [x] 1.1 Move the shared limits into `limits.rs` and point every user at
  it.
- [x] 1.2 Move `DatabaseSession` and the bounded call helpers into
  `session.rs`, with their tests in `src/tests/session.rs`.
- [x] 1.3 Move the command controller into `app.rs`, with its tests in
  `src/tests/app.rs`, leaving `main.rs` the entry point.

## 2. Verification

- [x] 2.1 Check that no module imports an item from the crate root.
- [x] 2.2 Run formatting, Clippy with warnings denied, workspace tests,
  rustdoc with warnings denied, the bounded fuzz checks, and strict
  OpenSpec validation.
