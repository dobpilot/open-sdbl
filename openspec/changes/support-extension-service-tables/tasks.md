## 1. Inventory (blocks all later phases)

- [x] 1.1 Capture SchemaStorage declarations, DBNames entries, and live
  catalog shapes for extra dimensions, calculation-kind dependency
  tables, change registration, recalculation, data history, and segment
  tables from reference PostgreSQL and MSSQL bases; store fixture blobs
  under `tests/` and a structure reference under `docs/`.
- [ ] 1.2 Document where extension declarations live (extension Config
  resources, extension DBNames entries) and how an extension-added
  attribute maps to `…X1` physical columns, with fixtures.
- [x] 1.3 Decide and record per family: full query support, resolve-only
  support, or explicit descope (expected descope candidates:
  `_DataHistory*`, `_DbSegments*`).

## 2. SchemaStorage projection

- [x] 2.1 Generalize `project_inline_table` to a table of inline kinds
  (extra dimensions, dependency tables, change registration,
  recalculation) with owner linkage and synthesized owner-reference
  columns; unknown kinds still produce `SchemaAnomaly`. Fixture tests
  per kind.
- [x] 2.2 Keep the differential streaming/tree oracle tests passing for
  the extended projection.

## 3. Metadata resolution

- [x] 3.1 Add service `MetadataKind` variants and DBNames alias mapping
  per the inventory; ownership resolves through DBNames numbers, never
  by parsing physical names as authority. Fixture tests.
- [x] 3.2 Implement extension declaration parsing and
  `resolve_metadata_with_extensions`; extension-added attributes merge
  into the owning object with extension origin recorded, mapped to
  `…X1` columns. Tests: extended object exposes the added attribute;
  base-only resolution is unchanged.
- [x] 3.3 Verify the resolution report on the extended fixture: resolved
  families no longer appear; a genuinely undeclared table still does.

## 4. Query compilation

- [x] 4.1 Compile `<ВидОбъекта>.<X>.Изменения` (bilingual) on the
  registered object into its change-registration table with node
  reference, message number, and the object's key columns; golden SQL
  on both dialects.
- [x] 4.2 Compile leading/base/displaced calculation-kind sources as
  tabular sections of the chart of calculation kinds; golden SQL on
  both dialects.
- [x] 4.3 Expose extra dimensions on chart-of-accounts sources with the
  spelling chosen in the inventory; golden SQL on both dialects.
- [x] 4.4 Extension-added attributes compile as ordinary fields of the
  extended object (projection, WHERE, dereference, ORDER BY); tests on
  both dialects including a field that exists only in the extension.
- [x] 4.5 Resolve-only families are reachable through metadata discovery
  but produce a clear typed diagnostic when used in FROM.

## 5. CLI integration

- [x] 5.1 Add SELECT-only extension-resource queries for both providers
  and fetch them in the unified pipeline when extensions are present.
  The verified `ConfigCAS` row layout is captured in
  `tests/fixtures/service_tables/live/column_types.tsv`; decoding the internal
  content-addressed graph and mapping it to `X1` remains task 1.2. The
  pipeline passes the opaque resources through its extension-decoder boundary.
  Acquisition intentionally polls `ConfigCAS` unconditionally because the
  captured base resources do not provide a separate authoritative
  extension-presence marker: determining whether extensions are present is
  the decoder's responsibility. The poll is a `SELECT` restricted to
  part-zero rows, runs inside the existing read-only transaction and timeout
  limits, and an absent/unsupported extension set becomes an empty `Vec` at
  the opaque `read_extensions` boundary. Until blocked task 1.2 is completed,
  that boundary deliberately returns an empty `Vec` for every input.
- [x] 5.2 REPL completion and metadata discovery include the new
  sources, with change-registration and dependency tables listed under
  their owners.

## 6. Parser conformance from reference dumps

Gaps surfaced while capturing the MSSQL (192.168.122.222) and PostgreSQL
(192.168.166.15) `demo` bases in phase 1. Each item is a real form the
parser handles today but that no test pins. Store large blobs
raw-DEFLATE-compressed and inflate them in the test (the DBNames dump is
already compressed; compress the ~2 MB SchemaStorage rather than
committing plaintext).

- [x] 6.1 Whole-`SchemaStorage` golden on both providers: parses without
  a `MetadataError`, the set of column type tags is exactly
  `{B,L,N,R,S,T,V}` (empirically the full alphabet in both bases, so the
  `KNOWN_COLUMN_TAGS` whitelist is complete on real data and
  `UnknownColumnTag` stays synthetic), and no `InvalidSchemaDeclaration`
  is produced.
- [x] 6.2 Whole-`DBNames` golden on both providers: all aliases (142 MSSQL,
  108 PostgreSQL) parse; the entries sharing the all-zero GUID (65/60)
  (`SystemSettings`, `Consts`, `AccumRgOpt`, …) parse and do **not**
  resolve to metadata objects; and neither those entries nor the 1160/2480
  non-zero GUIDs that legitimately repeat across `Fld`/`VT` entries
  produce a spurious `DuplicateGuid` finding. (Regression guard for the
  harden phase-3 duplicate-GUID heuristic against real data.)
- [x] 6.3 Live-catalog shape tests from captured columns: the composite
  exchange-plan node reference (`_NodeTRef binary(4)` +
  `_NodeRRef binary(16)`) collapses to one logical `Node` field; the PG
  `mvarchar` domain resolves through the actual `CATALOG` query result,
  not `information_schema`'s `USER-DEFINED`; and `binary(4/16)`,
  `numeric(p,s)`, `varbinary(max)`, `timestamp` all map correctly.
- [x] 6.4 Cross-provider differential: shared logical objects captured from
  the MSSQL and PostgreSQL bases retain their GUID/alias identity across
  provider-local DBNames renumbering; canonical recasing and logical field
  sets are compared using the fixtures from task 1.1.
- [x] 6.5 Scale/robustness: resolving a snapshot on the order of the
  PostgreSQL base (~3900 tables, tens of thousands of fields) completes,
  and the snapshot fingerprint is deterministic across two resolutions
  of identical inputs (guards the `format!("{:?}")` fingerprint noted in
  the CLI hardening review).
- [x] 6.6 Encoding goldens on real blobs: UTF-8-with-BOM `SchemaStorage`
  (both providers) and raw-DEFLATE `DBNames` decode to the expected
  logical text.

## 7. Verification

- [x] 7.1 Conformance test on the extended fixture base (resolution,
  report, queries) passes on both providers.
- [x] 7.2 fmt, clippy `-D warnings`, workspace tests, rustdoc, and
  `cargo audit` pass; `openspec validate
  support-extension-service-tables --strict` passes.
- [x] 7.3 README documents extension support and the new query sources.
