# Service-table and extension inventory (phase 1 of `support-extension-service-tables`)

Captured 2026-09-04 from a live 1C 8.3 base (`demo`, MSSQL 192.168.122.222,
platform-generated schema with two installed extensions). Fixture excerpts
live in `tests/fixtures/service_tables/`; the raw capture method is an
ignored test `review_inventory_probe` (temporary, to be replaced by a
dump subcommand or removed).

## Headline findings

1. **Almost every "undeclared" service table IS declared in
   SchemaStorage.** The resolution-report noise comes from two library
   gaps, not from missing platform data:
   - `project_inline_table` only accepts inline names starting `VT`,
     while the platform uses the same `"I"` inline form with arbitrary
     names (`ExtDim3937`, `BaseCK`, `LeadingCK`, `DisplacedCK`,
     `ExtProps`).
   - `MetadataKind::from_alias` knows 15 aliases while this base's
     DBNames uses ~130. Everything else resolves to no object, so its
     declaration never lands in `declared_names`.
2. **Extension tables (`…X1`) are genuinely undeclared** in the base
   SchemaStorage and absent from the base DBNames (0 matches for `X1`).
   Their declarations live in extension metadata: `_ExtensionsInfo`
   holds a small binary info record per extension
   (`_ExtensionZippedInfo`: GUID header + UTF-16LE brace text tail,
   NOT deflate), and the actual extension configuration is stored
   content-addressably (`ConfigCAS`); mapping attribute → `X1` column
   requires reading that store (task 1.2, still open).

## Declaration forms (fixtures)

All excerpts are verbatim SchemaStorage fragments.

### Top-level `"N"` families (already projected by `project_table`)

- **Change registration** — `accumrg_chngr.txt`
  (`{"AccumRgChngR1273","N",1273,…}`): columns `Node`
  (`R`-ref, class 4 = exchange plan), `MessageNo`, then the registered
  object's key columns. One ChngR table per registered object; DBNames
  aliases exist per kind: `ReferenceChngR`, `DocumentChngR`,
  `InfoRgChngR`, `AccumRgChngR`, `AccRgChngR`, `CRgChngR`, `ChrcChngR`,
  `CKindsChngR`, `BPrChngR`, `TaskChngR`, `SeqChngR`, `ConstChngR`,
  `ConstsChngR`, `AccChngR`, `ExtensionsChngR`, `ExtsChngR`,
  `ConfigChngR`, `CRgRecalcChngR`.
- **Recalculation** — `crg_recalc.txt`
  (`{"CRgRecalc3975","N",3975,…}`): `Recorder` (`R`, class 4),
  `CalcKind` (`R` → `CKinds<N>`), one `R`-column per register
  dimension, plus a data-separator field. DBNames alias `CRgRecalc`.
- `ExtsChngR` (`exts_chngr_with_extprops.txt`) additionally carries an
  **inline `"I"` child named `ExtProps`** with a single `FileName`
  column — the source of the `_ExtsChngR_ExtProps` /
  `_ConfigChngR_ExtProps` report lines.

### Inline `"I"` families (require generalizing `project_inline_table`)

Form is identical to `VT*`: `{"<Name>","I",0,"<Owner>",{cols},{…},{indexes},…}`,
physical name `_<Owner>_<Name>`, owner key column `ID` synthesized like
the `VT` owner reference.

- **Accounting extra dimensions** — `acc_extdim_inline.txt`:
  `ExtDim3937` owned by `Acc3930`; columns `LineNo`, `DimKind`
  (`R` → `Chrc<N>`), `DimIsMetadata`, `TurnoverOnly`, flags; index
  `ByLineNo(ID, LineNo, DimKind)`. DBNames alias `ExtDim` with its own
  number.
- **Calculation-kind dependencies** — `ckinds_baseck_inline.txt`,
  `ckinds_leadingck_inline.txt`: `BaseCK` / `LeadingCK` / `DisplacedCK`
  owned by `CKinds<N>`; columns `<Name>LineNo`, `<Name><RefCol>`
  (`R` → `CKinds<N>` self-reference), `Predefined<Name>TableLine`
  marker.

## DBNames alias census (this base)

~130 aliases. Beyond the 15 supported: per-kind `*ChngR` (see above),
`ExtDim`, `CRgRecalc`, `SeqB`, `Turnover`, `TurnoverDt`, `TurnoverCt`,
`AccumRgT/Opt/Agg*`, `AccRgCT/ED/Opt`, `*Opt`, `*SInf`, `CKindsDN`,
`BPrPoints`, `DocumentJournal`, `Consts`, `DataHistory*` (5 aliases),
`DbSegments`, `DbSegmentsItems`, `DbCopies*` (8), `ExtensionsInfo`,
`ExtensionsRestruct`, settings/service stores (`*Settings`,
`UsersWorkHistory`, `ScheduledJobs`, `IntegService*`, `STT*`,
`LangModel`, `Acoustic`, `Bots`, `Ecs`, …).

Numbers in DBNames are the physical numbers; ChngR numbering matches
the registered object's number (`AccumRgChngR1273` registers
`AccumRg1273`-family object — to be confirmed against a second base in
task 3.1 before relying on it).

## Encoding notes

- `SchemaStorage.CurrentSchema` on MSSQL is UTF-8 **with BOM** (the
  library's existing BOM handling covers this).
- `Params.DBNames` is raw DEFLATE as expected; decompressed cleanly by
  the same algorithm the library uses.
- `_ExtensionsInfo._ExtensionZippedInfo` is NOT deflate at any offset
  (probed 0..40, wbits −15/15/47); layout ≈ 16-byte GUID + counters +
  UTF-16LE brace-text tail. Treat as opaque until task 1.2.

## PostgreSQL reference base (192.168.166.15/demo, admin1c)

A second, larger configuration (3927 public tables) — complementary to
the MSSQL base:

- **Extension tables (`…x1`): 79 present** (MSSQL base had none). For all
  but one they are column-identical twins of their base table (extension
  installed, no attributes added). The exception is the key fixture:
  - `_reference14574x1` has **16 columns vs 13** in base
    `_reference14574`. The three extras — `_fld16536` (mvarchar),
    `_fld16537` (numeric), `_fld16538rref` (reference) — are attributes
    added by an extension. Their numbers (16536-16538) appear in
    **neither** the base DBNames nor the base SchemaStorage
    (`_reference14574` is declared there with exactly its 12 base
    attributes). This is the concrete proof that extension-added
    attributes require reading extension metadata (design task 1.2), and
    the acceptance target for phase 3.
  - The base `_reference14574` holds 0 rows while `_reference14574x1`
    holds 8: the object's data lives in the extension table.
- **No accounting / calculation-kind objects** in this base, so ExtDim /
  BaseCK / LeadingCK / DisplacedCK / CRgRecalc come only from the MSSQL
  base. The two bases together cover every target family.
- **Change registration** confirmed dialect-portable: PG
  `_referencechngr…` has `_nodetref`+`_noderref` (composite node
  reference), `_messageno`, `_idrref`, `_fld731` — same shape as the
  MSSQL `_…ChngR` columns, so one projection serves both providers.
- Encodings match the library's assumptions: PG `params.binarydata`
  DBNames is raw DEFLATE; `schemastorage.currentschema` is UTF-8 with
  BOM.

Fixtures: `tests/fixtures/service_tables/pg/`
(`ext_reference_base_columns.tsv`, `ext_reference_x1_columns.tsv`,
`ext_reference_x1_rows.tsv`, `reference14574_base_decl.txt`,
`referencechngr_columns.tsv`). Baseline test:
`tests/service_table_fixtures.rs::extension_adds_columns_absent_from_the_base_declaration`.

## Descope decisions recorded (task 1.3)

- `DataHistory*`, `DbSegments*`, `DbCopies*`, settings stores, `STT*`,
  `LangModel`, `Acoustic`, `Bots`: resolve-only (typed metadata +
  discovery, no query spelling) — matches design phase 4.5.
- Full query support targets: `*ChngR` (change registration), `ExtDim`,
  `BaseCK`/`LeadingCK`/`DisplacedCK`, `CRgRecalc` (resolve + owner
  linkage; query spelling per design phase 4).
- Extension attribute → `X1` mapping: blocked on the ConfigCAS reading
  (task 1.2); `X1` union of *base-declared* columns already works.
