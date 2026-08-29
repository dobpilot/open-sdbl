# Design: Initial SDBL lexer

## Context

The project starts without compatibility history or an existing parser. A
lexer is the narrowest useful first layer: it can support highlighting,
formatting, parsing, and diagnostics without deciding the full grammar.

## Goals and non-goals

Goals:

- Preserve byte spans and one-based source positions.
- Handle Unicode identifiers, including Cyrillic names.
- Keep the library deterministic and free of production dependencies.
- Make failures inspectable from both Rust and the CLI.

Non-goals:

- Parsing statements or validating query semantics.
- Claiming full compatibility with every 1C platform version.
- Normalizing identifier spelling in returned tokens.

## Decisions

`src/lib.rs` owns tokens, diagnostics, keyword classification, and the lexer.
`src/main.rs` only handles arguments and I/O. Tokens borrow slices from the
input, avoiding copies while keeping exact original spelling.

Byte offsets are zero-based half-open ranges. Lines and columns are one-based
Unicode scalar positions. Whitespace is discarded; comments are observable
tokens so downstream formatters can preserve them.

The CLI prints a stable tab-separated text form with position, kind, and an
escaped lexeme. This initial form is intentionally simpler than a versioned
serialization format.

## Risks

The initial subset follows the 1C:Enterprise Developer Guide: string literals
use double quotes and comments begin with `//`. Exact behavior can still differ
across platform versions, so the README does not make a complete compatibility
claim.
