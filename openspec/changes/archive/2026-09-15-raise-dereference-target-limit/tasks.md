## 1. Limit

- [x] 1.1 Raise `MAX_DEREFERENCE_TARGETS` to 256 and explain where the
  number comes from.
- [x] 1.2 Update the test that asserts the refusal.

## 2. Verification

- [x] 2.1 Re-record the corpus, check that the live server plans every
  compiled statement, and update the documentation.
- [x] 2.2 Run formatting, Clippy with warnings denied, workspace tests,
  rustdoc with warnings denied, the bounded fuzz checks, and strict
  OpenSpec validation.
