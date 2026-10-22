## Context and goals
The root library remains deterministic, I/O-free and free of production
dependencies. Database providers own acquisition, limits and connection cleanup.
The goal is to make those boundaries visible in internal types and modules while
preserving the existing public structs and all measured compatibility behavior.

## Decisions
1. Build on the repair's `ScalarStringEquality` helper in
   `src/query/core/codegen/expression.rs`. Consolidate remaining comparison
   eligibility/contextual exceptions and exact projection availability shared
   with `sources.rs`, `select.rs` and `projection.rs`. Both source-free and sourced compilation
   call them after resolving operand kinds. Render provider SQL separately;
   neither bypass access checks nor infer semantics by parsing emitted SQL.
   Do not unify all compiler contexts in one large abstraction.
2. Move native empty-role recognition and validation from
   `crates/open-sdbl-db/src/extensions.rs` into `src/metadata/roles.rs`.
   Construct an empty typed RoleRights value
   internally, rather than decoding a synthetic compressed fixture. Unknown,
   malformed and genuinely empty roles stay distinct. Keep resource naming,
   fetching and provider errors in the database crate. Prefer existing decoder
   entry points; any necessary new public API requires a spec amendment first.
3. Split `src/metadata/runtime_metadata.rs` projection by existing format/fact families. Parse
   and validate a family-specific internal record, then apply it to runtime facts.
   Retain ConfigDescriptor as the public compatibility input. Explicitly model
   absent, recognized and malformed states where Option fields currently blur
   that distinction. Preserve existing diagnostics and partial-result behavior:
   staging must not silently turn per-record validation into all-or-nothing input.

## Migration and validation
Land the review repairs first. Extract comparison/projection policy with parity
tests across source-free/sourced queries, dialects, NULL/Undefined and contextual
CASE. Move empty-role decoding with fixtures for empty, nonempty and malformed
roles plus extension acquisition tests. Split runtime projection one fact family
at a time, comparing complete runtime facts and diagnostics on existing native
fixtures. Run the full query corpus and workspace checks after integration.

## Risks and alternatives
A wholesale compiler or public descriptor rewrite would expand compatibility
risk and is excluded. Module extraction alone would leave invalid intermediate
states and duplicated policy; pair extraction with narrowly scoped internal types.
Do not normalize diagnostics, broaden format heuristics or regenerate expected
fixtures merely to make parity tests pass. Document any discovered behavior
change in a separate validated change before implementing it.
