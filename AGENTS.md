# Repository guidelines

This repository uses Rust 2024. Keep the root `open-sdbl` library free of
production dependencies and I/O. Runtime and database dependencies belong only
to application crates such as `crates/open-sdbl-cli`; keep the lexer and
metadata decoder independent from CLI I/O.

Before changing observable behavior, inspect `openspec/specs/` and active
entries in `openspec/changes/`. Public API, CLI, syntax, diagnostics, and
compatibility changes require a validated OpenSpec change before code.

Run workspace formatting, Clippy with warnings denied, tests, rustdoc with
warnings denied, and strict OpenSpec validation before committing.
