## Context

`compile_cross_source_join_equality` resolves both sides with
`resolve_direct`, and `validate_direct_join_condition_fields` rejects any
multi-segment path. Dereference joins live in `SourceScope::reference_joins`
and are emitted by `append_joined_reference_joins` after the last native
join: `FROM a JOIN b ON … LEFT JOIN __right_ref1 ON …`. SQL evaluates an
`ON` clause against the tables joined so far, so a reference join must
precede the `ON` that uses it.

## Decisions

### Resolution

- Both sides of a candidate anchor are resolved with `context.resolve`,
  which plans the dereference join on the path's base scope. The scope of a
  dereferenced path is its base source's scope, so `check_join_scope` and
  the anchor rule (one side on the joined scope, the other earlier) apply
  unchanged.
- `validate_direct_join_condition_fields` accepts one-hop paths; paths
  deeper than one hop keep the existing diagnostic.
- Dereference through a composite field inside `ПО` follows the rules of
  `dereference-composite-references` once that change lands; until then it
  reports the missing-target diagnostic.

### Rendering

- `CompilationContext` records `dereference_in_join` when a condition
  resolves a dereference. `compile_native_join` then renders every scope
  that owns reference joins as `(relation AS alias LEFT JOIN target AS
  __ref ON …)`; scopes without reference joins stay bare. Parenthesized
  joined tables are valid in PostgreSQL 9.0 and SQL Server 2008.
- Without the flag the flat form is kept, so existing goldens do not move.
- `compile_directional_full_join` rejects conditions with dereferences
  ("FULL JOIN condition supports direct fields only"): the transposition
  duplicates the condition into two branches whose anchor markers must be
  direct columns.

### Alternatives considered

Reordering the flat join list so that reference joins of source `k`
follow native join `k` breaks the `LEFT JOIN` semantics of later inner
joins and changes every existing statement; grouping is local and opt-in.
