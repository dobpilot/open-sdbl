## Context

A joined field path resolves to a side, field, and SQL alias. For a one-hop
dereference the alias belongs to an auxiliary join, but presentation compilation
currently rejects every alias other than the original source alias. The shared
join plan also implicitly qualifies its source column with the original source,
which cannot represent a chained join.

Presentation planning runs twice: first to collect target object IDs, then to
compile with application-provided structured plans. Both passes must resolve
the same aliases and join order.

## Goals / Non-Goals

**Goals:**

- present a reference field obtained by one supported dereference;
- present scalar dereferenced fields without an unnecessary second join;
- reuse the ancestor join when another projection already created it;
- preserve deterministic aliases in collection and strict compilation passes;
- support PostgreSQL and MSSQL.
- resolve SchemaStorage universal references without joining every metadata
  table or scanning the complete owner table before the main query.

**Non-Goals:**

- arbitrary-depth field paths;
- dereferencing inside an explicit ON condition;
- moving presentation policy into the core crate;
- raw SQL presentation templates.
- server-side dynamic SQL or joins to every possible reference target.

## Decisions

### Make the join source alias explicit

Each auxiliary join plan stores the alias containing its reference column.
SQL emission qualifies the source and type discriminator with that alias.
Deduplication includes the source alias, logical field, target relation, and
target discriminator so unrelated chains cannot collide.

### Carry owner identity with resolved joined paths

A resolved path carries its owning metadata object and whether its current SQL
alias represents that object's base identity. This makes presenting a
dereferenced `ID` reuse the current row and makes target discovery independent
of the original joined source.

### Keep the chain bounded

The parser and resolver still accept only one logical dereference hop. The
second auxiliary join is generated solely to evaluate a validated presentation
plan for the resulting reference value.

### Defer universal-reference presentations by result value

An empty SchemaStorage `R` target denotes a universal reference whose concrete
object type is stored in the row's `_RTRef` member. The main query emits one
opaque text payload containing the type discriminator and reference bytes for
that logical projection. `CompiledQuery` marks the corresponding output column
as deferred, so ordinary scalar text can never be mistaken for a payload.

The application groups the bounded result's payloads by resolved object ID and
asks the core to compile one safe batch lookup per object. Lookup SQL is built
from `MetadataSnapshot`, validated `PresentationPlan`, and caller-provided
reference bytes; applications do not construct identifiers or presentation
SQL. Missing target rows render as an empty presentation. Unknown runtime type
discriminators remain explicit errors.

This keeps `TOP`/`LIMIT` and all predicates in the main database query, avoids a
preflight scan of the physical owner table, and queries only target objects
actually present in the returned rows.

## Risks / Trade-offs

- A reference presentation can add another LEFT JOIN, but the join is bounded,
  metadata-resolved, and reused within its source side.
- Join order now matters for chained aliases; plans are appended ancestor-first
  during deterministic path resolution.
- Universal-reference presentation needs a second read after the main query;
  the CLI batches and caches presentation policy, but the two reads do not yet
  share one database transaction.
