## Compiler policy
- [ ] Inventory duplicated scalar comparison and projection-availability rules and their contextual exceptions.
- [ ] Extend the repair's shared equality helper to consolidate remaining contextual eligibility and projection-availability policies with explicit resolved-kind inputs and provider rendering boundaries.
- [ ] Add parity coverage for source-free/sourced paths, NULL/Undefined, contextual CASE and both dialects.

## Core role decoding
- [ ] Move native empty-role recognition and validation into the core role decoder without new production dependencies.
- [ ] Replace synthetic compressed role construction with an internal typed constructor.
- [ ] Test empty/nonempty/malformed native roles and database extension acquisition parity.

## Runtime metadata
- [ ] Identify format/fact families and pin existing facts, diagnostics and partial-result behavior in fixtures.
- [ ] Split projection into focused family modules and validate internal records before applying facts.
- [ ] Preserve the public ConfigDescriptor API and existing measured formats.

## Integration
- [ ] Verify public API, SQL corpus, diagnostics and runtime fact parity without automatic fixture regeneration.
- [ ] Run workspace formatting, Clippy, tests, rustdoc and strict OpenSpec validation.
