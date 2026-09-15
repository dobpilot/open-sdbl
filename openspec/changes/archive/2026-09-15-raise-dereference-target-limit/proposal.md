## Why

Dereferencing a composite reference is refused above 32 candidate targets,
a limit this project chose, not one the platform has: real configurations
dereference `ПодписанныйОбъект.Владелец` and `Объект.Наименование` over far
more objects, and the platform answers them.

Removing the limit and recompiling the corpus shows what it actually costs:
the two refused queries compile, the larger produces 65 KB of SQL with 188
`LEFT JOIN`s, and the live server plans every compiled corpus statement in
2.7 seconds total. The cost is real but ordinary; the refusal was too
tight.

## What Changes

- Raise the candidate limit to 256, the number of tables SQL Server accepts
  in one statement, and say so in the diagnostic's rationale.

## Capabilities

### Modified Capabilities

- `query-repl`: a composite dereference carries as many targets as a
  statement can join.

## Impact

Two corpus queries that were refused now compile. No API change.
