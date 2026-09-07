## Why

On a real information base the resolution report lists ~100 live tables
as "absent from SchemaStorage". They fall into two families the library
currently cannot read as first-class sources:

1. **Extension tables (`…X1`).** Configuration extensions create
   physical twins (`_InfoRg10038X1`, `_Document7811X1`, …). The compiler
   already unions them with their base table, but only for columns
   declared by the *base* configuration: attributes added by the
   extension itself have no declaration anywhere in the resolved
   snapshot, so they are invisible to queries, and the report flags
   every extension table as undeclared.
2. **Platform service structures.** Change-registration tables
   (`_InfoRgChngR…`, `_ConfigChngR_ExtProps`), accounting extra
   dimensions (`_Acc…_ExtDim…`), calculation-kind dependency tables
   (`_CKinds…_BaseCK/LeadingCK/DisplacedCK`), recalculation tables
   (`_CRgRecalc…`), data history (`_DataHistory…`), and segment tables
   (`_DbSegments…`) are real, queryable data the platform exposes in its
   own query language (e.g. `РегистрНакопления.X.Изменения`,
   `ПланВидовРасчета.X.ВедущиеВидыРасчета`, accounting extra
   dimensions), but our SchemaStorage projection only recognizes `VT*`
   inline tables and our DBNames mapping only covers the fifteen main
   object kinds.

The user requirement is explicit: these tables must be readable and
decodable, not filtered out of the report.

## What Changes

- **Inventory phase first.** Capture real SchemaStorage declarations,
  DBNames entries, and live-catalog shapes for every affected family
  from reference PostgreSQL and MSSQL bases; document each structure
  (keys, node references, dimension links) before writing projection
  code. Unknowns are resolved here, not guessed in code.
- **SchemaStorage projection** learns the remaining inline table kinds
  (extra dimensions, calculation-kind dependency tables,
  change-registration and recalculation tables) instead of silently
  skipping declarations that are not `VT*`.
- **DBNames/metadata resolution** maps the service-table aliases and
  derived names so these tables resolve to typed metadata objects with
  queryable fields, and extension declarations (the extension side of
  Config/DBNames) merge extension-added attributes into the owning
  object so `…X1` columns become queryable.
- **Query compilation** exposes the new sources through their native 1C
  query-language spellings (change registration as
  `<ВидОбъекта>.<X>.Изменения` on the registered object,
  leading/base/displaced calculation kinds
  as tabular sections of the chart of calculation kinds, accounting
  extra dimensions on chart-of-accounts sources), for both PostgreSQL
  and MSSQL through the existing dialect layer.
- **Resolution report** stops flagging tables that are now declared;
  what remains in the report is a real mismatch.
- REPL completion and metadata discovery commands surface the new
  sources.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `onec-metadata`: project all inline SchemaStorage table kinds; resolve
  service tables and extension-added attributes as typed metadata.
- `query-compilation`: compile the new sources with dialect parity.
- `query-repl`: discovery and completion cover the new sources.

## Impact

- Library-only feature work; no new dependencies expected.
- The resolution report becomes quieter as a side effect of real
  support, not filtering.
- Query subset grows: new FROM spellings for change registration,
  calculation-kind dependencies, and extra dimensions.
- Extension support changes `resolve_metadata` inputs: the CLI pipeline
  must additionally fetch extension Config resources where present.
