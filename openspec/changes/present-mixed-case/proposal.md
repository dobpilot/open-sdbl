## Why

`ПРЕДСТАВЛЕНИЕ(ВЫБОР … ТОГДА <строка> ИНАЧЕ <ссылка> КОНЕЦ)` is refused —
either with "expression kinds differ" or, when a branch reads a composite
field, with "compound field can be projected but not used in expressions".
A corpus query of a real configuration builds exactly that shape: it
chooses among parameter strings by the value of a composite reference and
falls back to the reference itself.

The platform answers it branch by branch. Measured on the probe base
against 8.3.27: the string branch answers the string, the reference branch
answers the presentation of the reference, and a `NULL` value answers
`NULL`.

## What Changes

- Push `ПРЕДСТАВЛЕНИЕ` into the branches of a `ВЫБОР` instead of asking
  for one kind across them: every branch is presented on its own and the
  result is one string column.
- Keep refusing when a branch needs the deferred protocol — a reference
  expression that is not a field — because a column is either deferred as
  a whole or built from a plan.

## Capabilities

### Modified Capabilities

- `query-repl`: a presentation may be taken of a `ВЫБОР` whose branches
  differ in type.

## Impact

One corpus query compiles. Generated SQL gains a `CASE` whose branches are
presentations; nothing else changes.
