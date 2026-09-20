# Design — the field-usage report

## Context

`Prepared` already answers two requests collected on one pass:
`PresentationRequest` and `RestrictionRequest`. Both are filled while the
batch compiles unbound, and both are read by the application before it
compiles for real. The field usage fits that shape exactly.

The compiler resolves every field read through one of a few places that
build a `ResolvedPath`, which knows the scope, the object the path ended
on, and the field index. What it does not know is *why* the field is being
read — that is a property of the clause being compiled.

## Decisions

### 1. The role comes from the clause, the identity from the path

`CompilationContext` carries the role of the clause it is compiling, set
around each of them in `compile_branch`: projections, group keys,
`ИМЕЮЩИЕ`, the join conditions, `ГДЕ`, and the ordering. The expression
compiler refines it: inside an aggregate's argument the role is
`Aggregate`, and a field read as part of a computed expression rather than
projected on its own is `Expression`.

The identity is the one the column origin already uses — object, section,
`FieldId` — so the two reports agree by construction, and a dereference is
reported against the object it ended on for the same reason.

### 2. One entry per field and role

A field read twice in one role is one entry; a field read in two roles is
two. The order is the order of first sight, which is stable for a given
source text.

### 3. Collected on the preparation pass

The catalog collects into a set while the batch compiles, exactly as it
collects restriction targets, and `prepare_query_with` hands the result to
`Prepared`. Nothing is added to the compilation for real, so no generated
SQL can change: the collection writes into a side table and is read by
nobody that renders.

## Risks

- A role the compiler forgets to set would report a field as part of an
  expression when it is really a predicate. The roles are therefore set at
  the clause boundaries in one function rather than sprinkled, and the
  tests assert each role against a statement that uses only that clause.

## No new dependencies

Confined to the core crate.
