## Why

`НастройкаСвязей.СсылкаИз В ИЕРАРХИИ (&СсылкаИз)` tests a composite
reference field, and the compiler refuses it: `В ИЕРАРХИИ` needs a field
that references exactly one catalog. A corpus query of a real
configuration stops there.

The platform accepts it. Measured on the probe base against 8.3.27 over a
catalog whose `Объект` attribute holds either a product or a client: the
predicate answers the rows whose value is a product under the named group,
and answers nothing for rows holding a client — the value has to be of the
seeds' type to be under them at all.

## What Changes

- Accept a composite reference field in `В ИЕРАРХИИ`, comparing its
  identifier member with the seeds and requiring its type member to be the
  type the seeds belong to.

## Capabilities

### Modified Capabilities

- `query-repl`: `В ИЕРАРХИИ` tests a composite reference too.

## Impact

One corpus query compiles. The generated predicate gains a type guard
beside the existing membership test.
