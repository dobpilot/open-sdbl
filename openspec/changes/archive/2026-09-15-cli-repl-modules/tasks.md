## 1. Split

- [x] 1.1 Create `repl/` with the loop in `mod.rs` and move completion,
  preparation, presentations, meta commands, rendering, description and
  terminal handling into modules of their own.
- [x] 1.2 Give each module the imports it needs, with no module importing
  from the crate root.

## 2. Tests

- [x] 2.1 Move the console tests into `src/tests/repl_<concern>.rs`,
  grouped by the module they cover.

## 3. Verification

- [x] 3.1 Run formatting, Clippy with warnings denied, workspace tests,
  rustdoc with warnings denied, the bounded fuzz checks, and strict
  OpenSpec validation.
