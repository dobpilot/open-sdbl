## Context

All two-source joins share one condition compiler. FULL JOIN is implemented as
two directional LEFT JOIN branches joined with `UNION ALL`; it uses a column
from a mandatory cross-source equality to remove the matched rows from the
second branch. Native INNER, LEFT, and RIGHT joins currently inherit the same
bounded condition shape.

The general joined-expression compiler already supports comparisons, IN-lists,
date functions, metadata values, and dialect-aware literals. It also supports
reference-property paths by scheduling auxiliary joins, but those joins are
appended after the main join and therefore cannot safely be referenced by its
ON clause.

## Goals / Non-Goals

**Goals:**

- compile the common `key equality AND status predicates` shape;
- preserve outer-join ON semantics instead of moving predicates to WHERE;
- retain a reliable FULL JOIN anti-match marker;
- reuse PostgreSQL and MSSQL scalar-expression generation.

**Non-Goals:**

- joins without any cross-source equality anchor;
- reference-property dereferencing inside ON;
- arbitrary OR trees containing the only join anchor;
- more than two explicit metadata sources.

## Decisions

### Flatten only top-level AND conjunctions

The condition compiler visits top-level `И`/`AND` members. A cross-source
direct-field equality at this level is compiled with the existing compound
reference-aware equality generator and may provide the FULL JOIN anti-match
marker. Other members are compiled as scalar predicates.

An equality nested under `OR` does not qualify as the anchor because it is not
mandatory for every matching row.

### Validate direct fields before scalar compilation

Every field path in an additional predicate is resolved with the direct-field
resolver before invoking the general joined-expression compiler. This permits
qualified and unqualified direct fields while preventing the compiler from
scheduling an auxiliary reference join that would be unavailable inside ON.

### Keep predicates in ON

Additional predicates are emitted in the main ON expression for native and
transposed joins. Moving a one-sided predicate to WHERE would change LEFT,
RIGHT, and FULL JOIN null-extension semantics.

## Risks / Trade-offs

- The mandatory equality anchor remains stricter than unrestricted SQL, but it
  bounds join shape and keeps FULL JOIN transposition correct.
- Reference-property predicates must still be expressed outside ON or through
  an explicit metadata source until auxiliary joins can be ordered safely.
