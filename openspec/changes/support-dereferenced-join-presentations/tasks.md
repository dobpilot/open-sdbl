## 1. Join planning

- [x] 1.1 Store an explicit source alias in auxiliary join plans.
- [x] 1.2 Carry owner identity and base-identity state in resolved joined paths.
- [x] 1.3 Include source alias and target relation in join deduplication.

## 2. Presentation compilation

- [x] 2.1 Compile scalar presentations from a dereferenced alias.
- [x] 2.2 Chain reference-presentation joins from the dereferenced alias.
- [x] 2.3 Preserve deterministic collection/strict passes and both SQL dialects.
- [x] 2.4 Emit marked deferred payloads for universal SchemaStorage references.
- [x] 2.5 Compile safe PostgreSQL and MSSQL batch presentation lookups.

## 3. CLI resolution

- [x] 3.1 Decode deferred payloads and resolve RTRef through the metadata
  snapshot.
- [x] 3.2 Batch returned reference IDs by object and reuse cached presentation
  plans.
- [x] 3.3 Replace deferred cells before rendering without changing visible
  column shape.

## 4. Verification

- [x] 4.1 Add unit tests for chained, reused, scalar, deferred, and dialect
  behavior.
- [x] 4.2 Verify the reported query against PostgreSQL `erp_ur`.
- [x] 4.3 Run formatting, Clippy with warnings denied, workspace tests, rustdoc
  with warnings denied, and strict OpenSpec validation.
