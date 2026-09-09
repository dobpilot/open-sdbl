## Context

`CompilationContext` keeps sources as `SourceScope`s addressed by `ScopeId`
and resolves qualified fields by scope; the single-join code
(`resolve_join_source`, `compile_join_condition_parts`,
`compile_native_join`) is written against two scopes. The FULL JOIN
transposition duplicates the branch into two `LEFT JOIN` statements, which
does not generalize to chains without nested statements.

## Decisions

### AST

`SelectAst::joins: Vec<JoinAst>` in source order. The parser loops over
`[ВНУТРЕННЕЕ|ЛЕВОЕ|ПРАВОЕ|ПОЛНОЕ] [ВНЕШНЕЕ] СОЕДИНЕНИЕ <source> [КАК alias] ПО <condition>`
until no join keyword follows. Each join charges one work unit plus its
condition.

### Scopes and conditions

Every join registers a new `SourceScope`; aliases must be unique across all
sources of the branch, and the same metadata object may appear under
different aliases. The anchor rule generalizes: the `ПО` condition of join
`k` must contain, at top level under `И`, at least one direct-field equality
between the joined source and some earlier scope (`0..k`). Other
conjuncts may reference any scope at or before `k`; referencing a later
scope is a positional diagnostic. Reference properties in `ПО` remain
unsupported, unchanged.

### Rendering

Native joins are emitted in source order:
`FROM s0 [alias] <JOIN k1 ON …> <JOIN k2 ON …> …`, followed by the
dereference/presentation `LEFT JOIN`s collected in the join cache. Placing
the reference joins after a `RIGHT JOIN` keeps semantics, because a `LEFT
JOIN` never removes rows. `RIGHT JOIN` chains keep SQL's left-to-right
association, which is also 1C's meaning.

`ПОЛНОЕ СОЕДИНЕНИЕ` keeps the current two-branch transposition and is only
accepted when it is the branch's sole join; a FULL join anywhere in a
multi-join branch is `UnsupportedFeature` with the message
`FULL JOIN must be the only join of a branch`.

### Projection and ordering

The joined-branch rules stay: `*` is rejected, ordering must use projected
fields, unqualified field names must be unique across all scopes
(ambiguity is `AmbiguousField`).

## Risks / Trade-offs

- Users wanting FULL joins in chains can wrap the FULL join in a nested
  query once `support-nested-queries` lands.
