## Context

`resolve_dereference` requires `QueryableField::reference_target`, the
unique SchemaStorage target of an `R` field. Composite fields carry
`_TYPE`, `_RTRef`, and `_RRRef` members and, for `ЛюбаяСсылка`, no
declared targets at all (`reference_targets` is empty, as `Объект` of
`СвязиОбъектов` shows). Derived sources and temporary tables expose such
fields as one 20-byte payload column. Presentations already plan guarded
joins per target (`ensure_presentation_join` with a `database_type` guard
and the shared `JoinKey`), and `MetadataObject::number` gives the type
number of every table.

## Decisions

### Candidate targets

1. Declared targets: `reference_targets` without empty entries, mapped to
   objects as `presentation_targets` does.
2. Otherwise, for a compound field with `_RTRef` or a runtime-typed derived
   column: every object of a reference kind (catalogs, documents, charts of
   characteristic types, accounts and calculation types, exchange plans,
   business processes, tasks, enumerations) whose queryable fields match the
   attribute token by name or alias. Each scanned object charges one work
   unit; the field cache keeps repeated scans cheap.
3. Candidates lacking the attribute are dropped; none left is
   `UnknownField` at the attribute token; more than 32 is
   `UnsupportedFeature` advising `ВЫРАЗИТЬ(... КАК Справочник.X)`.

### Joins and value

- One `JoinPlan` per candidate, deduplicated through `JoinKey` with the
  candidate's type number as `database_type`, so a presentation of the
  same field reuses the join. The source side is either the field's
  `_RRRef`/`_RTRef` members or, for a payload column, the payload split
  by new dialect helpers (`substring(x from 5 for 16)` /
  `SUBSTRING(x, 5, 16)` and the 4-byte prefix). `JoinPlan` gains a source
  form enum for the two shapes.
- The value is `CASE WHEN <type expr> = <type1> THEN <ref1>.<col1> WHEN …
  END`; a target without the attribute contributes no branch, so its rows
  yield `NULL`, matching the platform.
- `ResolvedPath` gains an optional rendered expression that `sql_column`
  returns instead of `alias.column`, so projections, filters, grouping
  keys, ordering, and (with the previous change) join conditions consume
  the dereference unchanged. The synthesized field has one column whose
  kind is the common kind: the same variant in every branch (else a
  positional diagnostic), string length the maximum, number precision and
  scale dropped, references widened to a runtime-typed payload with the
  union of targets, in which case every branch renders its own
  `RTRef ‖ RRRef` payload.
- `identity_is_base` is false and the owner is the derived placeholder;
  `ПРЕДСТАВЛЕНИЕ` of the dereferenced value and a second hop are rejected
  with `UnsupportedFeature`.

### Nested sources

The nested-query requirement no longer forbids dereferencing a
runtime-typed derived column: it follows the composite rules above with
the column's known targets as declared candidates.

## Risks

Scanning by attribute name can join many tables for common attributes such
as `Наименование`; the 32-candidate bound and the work budget keep the
statement bounded, and the diagnostic names the narrowing cast.
