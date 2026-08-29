# Repository guidelines

This repository uses Rust 2024 and has no production dependencies. Keep the
lexer in `src/lib.rs` independent from CLI I/O in `src/main.rs`.

Before changing observable behavior, inspect `openspec/specs/` and active
entries in `openspec/changes/`. Public API, CLI, syntax, diagnostics, and
compatibility changes require a validated OpenSpec change before code.

Run formatting, Clippy with warnings denied, tests, rustdoc with warnings
denied, and strict OpenSpec validation before committing.
