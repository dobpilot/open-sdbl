## Why

Normal 1C queries can assign names to projected columns and read a document or
catalog tabular section as `<kind>.<object>.<section>`. The current bounded
grammar stops at `КАК` after a projection and treats every third source segment
as a virtual-register table, so a valid joined query over a document tabular
section fails before metadata resolution.

## What Changes

- Accept explicit `КАК`/`AS` aliases after named, scalar, aggregate, and
  presentation projections and use the alias as the output column label.
- Resolve document and catalog tabular-section sources from the parent Config
  resource, the section's exact DBNames `VT` entry, SchemaStorage, and the live
  database catalog.
- Decode SchemaStorage inline-table declarations shaped as
  `{"VT<number>","I",0,"<parent>",...}` into their canonical
  `<parent>_VT<number>` table representation.
- Expose the tabular-section owner reference as `Ссылка`/`ID` and its numbered
  line field as `НомерСтроки`/`LineNo` without guessing unrelated tables.
- Support tabular sections in ordinary and joined SELECT branches, including
  existing one-hop reference-property resolution.
- Return positional diagnostics for missing, ambiguous, non-live, or malformed
  tabular-section mappings.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: compile projection aliases and authoritative tabular-section
  sources.

## Impact

- Extends the dependency-free query AST, metadata adapter, SchemaStorage
  decoder, PostgreSQL renderer, and MSSQL renderer.
- Reuses existing decoded metadata and does not add database I/O or production
  dependencies to the root library.
- Does not add deeper-than-one-hop reference paths or write operations.
