# Design

## Guiding constraints

- The core library stays dependency-free and I/O-free: all new inputs
  (extension Config resources) arrive as caller-provided blobs through
  the existing `resolve_metadata*` shape.
- No structure is implemented from guesswork: phase 1 produces a
  documented inventory (fixtures + notes) from real 8.3 bases on both
  providers, and every projection added later cites its fixture.
- Existing behavior is frozen by the current golden tests; new sources
  extend the query subset without changing generated SQL for existing
  queries.

## 1. Inventory (phase 1 output feeds everything else)

Use the CLI against reference bases to capture, for each family:

- the SchemaStorage declaration form (tag, nesting, key columns) for:
  extra dimensions (`_Acc<N>_ExtDim<M>`), calculation-kind tables
  (`_CKinds<N>_BaseCK/LeadingCK/DisplacedCK`), change registration
  (`_<Kind><N>ChngR…` and `_ConfigChngR_ExtProps`/`_ExtsChngR_ExtProps`),
  recalculation (`_CRgRecalc<N>`), data history (`_DataHistory*`), and
  segments (`_DbSegments*`);
- how DBNames names each family (alias strings and numbering, including
  the verified match between each `*ChngR` number and its registered
  object);
- where extension declarations live (extension Config resources,
  extension DBNames entries) and how an extension-added attribute maps
  to physical `…X1` columns;
- live-catalog column shapes on PostgreSQL and MSSQL.

Deliverable: fixture blobs under `tests/` plus a structure reference in
`docs/`, both used by later phases. Families whose structures cannot be
confirmed (candidates: `_DataHistory*`, `_DbSegments*`) are explicitly
descoped in this phase rather than half-implemented.

## 2. SchemaStorage projection

`project_inline_table` currently accepts only names starting `VT`.
Generalize to a table of inline kinds discovered in phase 1 — each with
its name pattern, owner linkage, and synthesized owner-reference
column, mirroring how `VT*` synthesizes `{parent}_IDRRef`. Unknown
inline kinds keep producing `SchemaAnomaly` findings (no regression of
the totality guarantees from `harden-core-reliability`).

## 3. Metadata resolution

- `MetadataKind` gains service kinds (change registration,
  recalculation, calculation-kind dependency, extra dimension) —
  `#[non_exhaustive]` already allows this without breaking callers.
- Derived-name resolution: a change-registration table is owned by the
  registered object and keyed by exchange-plan node columns; extra
  dimensions belong to their chart of accounts; dependency tables to
  their chart of calculation kinds. Ownership is resolved through
  DBNames numbers per the phase-1 findings, never by parsing physical
  names as authority (consistent with the existing DBNames-authoritative
  rule).
- Extensions: `resolve_metadata_with_extensions(...)` accepts the
  extension Config/DBNames blobs; extension-added attributes merge into
  the owning object's field list, marked with their extension origin,
  and map to the `…X1` physical columns. The compiler's existing
  extension-union logic then picks them up naturally; the CLI pipeline
  fetches extension resources when the base has any.
- The resolution report keeps `TableNotDeclared` for genuinely unknown
  tables; everything resolved above stops appearing by construction.

## 4. Query compilation

New sources, using native 1C query-language spellings and compiled
through the existing dialect layer (both backends, parity macros):

- `<ВидОбъекта>.<X>.Изменения` / `<ObjectKind>.<X>.Changes` —
  change registration attached to the registered object itself,
  projecting node reference, message number, and the object's key
  columns. Each registered object owns a separate `*ChngR` table; an
  exchange plan does not own one aggregate change-registration table.
- `ПланВидовРасчета.<X>.ВедущиеВидыРасчета` (and base/displaced
  variants) — as tabular-section-like sources of the chart of
  calculation kinds.
- Extra dimensions exposed on chart-of-accounts sources per the
  phase-1 structure (spelling decided with the inventory; the platform
  exposes them through accounting virtual tables).
- Extension-added attributes require no new syntax: they appear as
  ordinary fields of the extended object.

Recalculation, data history, and segment tables are read-only resolved
metadata first (visible in `\d`, queryable by physical shape); dedicated
query spellings are added only where the platform's own query language
has one.

## 5. CLI

- Pipeline fetches extension Config resources (new SELECT-only queries
  in `queries.rs`, both providers) when extensions are present.
- REPL completion and `\dt`/`\d` include the new sources; the
  completion catalog treats change-registration and dependency tables
  as children of their owners.

## 6. Testing

- Fixture-driven unit tests per projection (from phase-1 blobs).
- Golden SQL for each new source on both dialects via the existing
  parity macros.
- A conformance test resolving a fixture base with extensions asserts:
  extension attribute queryable, `…X1` tables no longer reported, and
  the report still catches a genuinely undeclared table.
