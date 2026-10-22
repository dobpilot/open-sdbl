## Compiler policy
- [x] Inventory duplicated scalar comparison and projection-availability rules and their contextual exceptions.
- [x] Extend the repair's shared equality helper to consolidate remaining contextual eligibility and projection-availability policies with explicit resolved-kind inputs and provider rendering boundaries.
- [x] Add parity coverage for source-free/sourced paths, NULL/Undefined, contextual CASE and both dialects.

## Core role decoding
- [x] Move native empty-role recognition and validation into the core role decoder without new production dependencies.
- [x] Replace synthetic compressed role construction with an internal typed constructor.
- [x] Test empty/nonempty/malformed native roles and database extension acquisition parity.

## Runtime metadata
- [x] Identify format/fact families and pin existing facts, diagnostics and partial-result behavior in fixtures.
- [x] Split projection into focused family modules and validate internal records before applying facts.
- [x] Preserve the public ConfigDescriptor API and existing measured formats.

## Integration
- [x] Verify public API, SQL corpus, diagnostics and runtime fact parity without automatic fixture regeneration.
- [x] Run workspace formatting, Clippy, tests, rustdoc and strict OpenSpec validation.

## Validation results
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: 1198 passed, 0 failed, 13 ignored (5 maintenance tests and 8 opt-in live database tests).
- `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps`: passed.
- `openspec validate --all --strict`: 47 passed.
- Existing SQL corpus passed unchanged; frozen complete decoder results matched for 461 native fixtures, including damaged collections and truncated streams.
- Existing public structs, production dependencies and fixture expectations are unchanged; the additive role decoder API is specified in this change.
