## Why

Platform queries routinely join a tabular section to its owner through a
dereference in the condition: `Документ.Корреспонденция.Корреспонденты КАК
К ПО К.Ссылка.Основание = ВходящийДокумент.Ссылка`. The compiler rejects
every reference property in `ПО` ("JOIN condition supports direct fields
only") because dereference joins are appended after all native joins,
where an `ON` clause cannot see them.

## What Changes

- A join condition MAY dereference one hop through a fixed single-target
  reference of the joined source or of any earlier source, both as the
  anchor equality and in additional predicates.
- When a condition uses a dereference, every source's dereference joins are
  rendered as a parenthesized group next to that source, so the `ON`
  clauses can reference them; statements without such conditions keep
  their current SQL.
- `ПОЛНОЕ СОЕДИНЕНИЕ` with a dereference in `ПО` is rejected.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: join conditions accept one-hop dereferences.

## Impact

- `src/query/core/codegen/select.rs` (join condition compilation and
  native join rendering), `context.rs` (a flag recording dereferences
  used by conditions), tests, README, `docs/query-language-support.md`.
- Depends on `widen-join-reference-equality` for widened anchors.
