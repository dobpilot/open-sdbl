# Change: Compile application-defined 1C value presentations

## Why

Reference presentation rules are application policy: the reusable SDBL
compiler can determine possible target metadata types, but it must ask its
caller which metadata fields and template represent each type. The protocol
must use stable metadata identities rather than localized names, and repeated
console queries must not repeatedly resolve the same policy.

Metadata name lookup is currently linear over snapshot vectors. Presentation
planning adds repeated object, attribute, field, and database-type lookups, so
the snapshot needs immutable hash indexes with expected constant-time lookup.

## What Changes

- Add GUID-backed object and attribute identifiers and numeric identifiers for
  standard fields.
- Add indexed lookup of metadata objects, attributes, fields, and database
  reference type numbers.
- Parse `.Представление`/`.Presentation`, `ПРЕДСТАВЛЕНИЕССЫЛКИ`/
  `REFPRESENTATION`, and `ПРЕДСТАВЛЕНИЕ`/`PRESENTATION`.
- Add a two-phase compilation callback contract: core emits one batch of
  possible reference target IDs, the application returns structured field and
  template plans, then core generates PostgreSQL SQL.
- Keep table and field names out of that contract. Only metadata GUIDs,
  numeric standard-field IDs, and literal template text cross the boundary.
- Add a bounded asynchronous Moka cache in the CLI provider. The dependency
  does not enter the core crate.

## Impact

- Affected specs: `query-repl`, `onec-metadata`, `crate-architecture`.
- Affected code: core metadata resolver/query compiler and CLI console.
- Existing non-presentation query compilation remains source-compatible and
  deterministic.
