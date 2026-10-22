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
   entry points for `.0` rights resources. Those APIs cannot validate a
   descriptor against its expected role GUID. Add the narrowly scoped
   `parse_role_without_rights(&Guid, &[u8]) -> Option<RoleRights>` entry point:
   it uses the default decoded-byte limit and returns None for malformed,
   unknown, borrowed, mismatched or rights-bearing descriptors. This preserves
   the database loader's existing unreadable-role behavior and adds no format.
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

## Implementation inventory

`codegen/scalar_policy.rs` owns comparison eligibility (PostgreSQL, `=`, no
contextual CASE), classification from resolved scalar kinds, and the exact
scalar-projection rule for Undefined. Name resolution and authorization remain
at the original call sites. `expression.rs` renders the selected SQL policy;
`ast::expression_children` owns shared traversal without a source-context dependency.
Non-equality, SQL NULL, contextual CASE and MSSQL retain their existing paths.

Runtime projection now has these private boundaries:

| Module | Responsibility |
| --- | --- |
| `runtime_metadata.rs` | Object-envelope recognition and ordered dispatch |
| `values` | Strict shapes and Absent/Malformed/Known collection recognition |
| `attributes`, `sections` | Profile validation and application of complete checked collections |
| `catalogs_documents` | Legacy/current families and independent typed document settings |
| `objects` | Reports, processors, registers and reference object profiles |
| `configuration` | Functional options, common attributes and their resolution |
| `fill_checking` | Object and attribute fill-checking rules |
| `named_types`, `identity` | Named-type resolution and header-only projection |

`ValidatedAttributes` and `ValidatedSections` keep descriptor indices private;
they apply only to the same slice before any reordering. Whole-collection
validation precedes publication. Existing independently valid facts remain
available when another setting is malformed; public None/empty semantics are
unchanged. The public ConfigDescriptor and runtime-facts types are unchanged.

Whole decoder results were frozen for 461 existing native DEFLATE fixtures
before moving runtime code. The parity test covers original inputs, damaged
final collections and truncated compressed streams, including field order,
unknown/empty distinctions and diagnostics. It supplements the existing native
XML and targeted malformed-record assertions. Existing fixtures and expected
SQL results are not regenerated.
