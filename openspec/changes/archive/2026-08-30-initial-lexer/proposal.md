# Proposal: Initial SDBL lexer

## Why

Tools for the 1C query language need a small, reusable lexical foundation.
The project also needs a command-line entry point that makes the library easy
to inspect in scripts and during development.

## What changes

- Add a dependency-free Rust library that tokenizes SDBL source text.
- Recognize Russian and English keyword aliases without case sensitivity.
- Report precise, human-readable diagnostics for malformed input.
- Add an `open-sdbl lex` command for files and standard input.
- Document the supported subset, compatibility limits, and development checks.

## Capabilities

### New capabilities

- `sdbl-lexer`: lexical analysis API and command-line interface.

## Impact

- Creates the initial public Rust API and CLI contract.
- Introduces no production dependencies outside the Rust standard library.
