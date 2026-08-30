## Context

`SchemaStorage` already exposes `ColumnType { tag: "R", reference_target }`.
Resolved query fields currently discard that relationship and parse at most
`qualifier.field`, so `Организация.Код` is misclassified as an invalid source
qualifier.

## Decisions

### Distinguish qualification from dereference by the source name

A two-segment path is a qualified local field only when its first segment is
the explicit source alias or source object name. Otherwise it is interpreted
as `<reference-field>.<target-field>`. Three segments mean
`<source-qualifier>.<reference-field>.<target-field>`. Longer paths fail in
this bounded version.

### Resolve joins from SchemaStorage only

The source field must have exactly one `R` reference target. The compiler maps
that canonical target table to a resolved live metadata object, finds its `ID`
physical member, and joins the source `RRef` member to it. Missing or ambiguous
relationship information is an error; no name or table-prefix heuristic is
used.

### Generate shared LEFT JOINs

Each distinct source reference field gets one stable generated alias and one
`LEFT JOIN`, reused across projection, filter, and order expressions. A left
join preserves a source row when its optional reference is empty.

## Risks / Trade-offs

- Dynamic references with more than one target cannot produce one static join
  and are rejected.
- Only one dereference hop is supported initially.
