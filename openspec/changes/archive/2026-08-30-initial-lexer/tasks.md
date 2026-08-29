## 1. Project foundation

- [x] 1.1 Create the Rust 2024 package, repository metadata, and user documentation.

## 2. Lexer

- [x] 2.1 Add failing tests for keywords, identifiers, parameters, literals, comments, spans, and diagnostics.
- [x] 2.2 Implement the public tokenization API without production dependencies.

## 3. CLI

- [x] 3.1 Add tests for file and standard-input workflows.
- [x] 3.2 Implement `open-sdbl lex [FILE|-]` and help output.

## 4. Verification

- [x] 4.1 Run `cargo fmt --all -- --check`.
- [x] 4.2 Run `cargo clippy --all-targets -- -D warnings`.
- [x] 4.3 Run `cargo test` and rustdoc with warnings denied.
