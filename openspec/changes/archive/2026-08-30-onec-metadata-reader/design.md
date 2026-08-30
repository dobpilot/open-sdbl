## Context

The crate currently has a dependency-free lexer in `src/lib.rs` and an I/O-only
CLI in `src/main.rs`. The 1C platform stores the required mapping in three
formats: raw-DEFLATE configuration blobs, plaintext brace serialization, and
lowercase PostgreSQL catalogs. DaJet Metadata provides useful evidence for the
resource loading and brace-reader behavior, while the mapping contract in the
new specification is stricter about DBNames authority and SchemaStorage.

## Goals / Non-Goals

**Goals:**

- Keep all decoding, parsing, normalization, and resolution deterministic and
  testable without a database.
- Preserve the existing zero-Cargo-dependency policy.
- Keep database process execution and output formatting out of the lexer and
  metadata core.
- Make malformed authoritative resources fail visibly and with useful context.

**Non-Goals:**

- Reimplement the 1C runtime, configuration editor, or authentication system.
- Infer logical metadata from PostgreSQL table-name patterns.
- Support MSSQL acquisition in this change; the core model remains DBMS-neutral
  enough to add it later.
- Decode modules, forms, rights, pictures, or other `<guid>.<part>` resources.

## Decisions

### Use a small general brace-value parser

The metadata module will parse atoms, quoted strings, and nested lists into a
generic value tree. DBNames, Config descriptors, and SchemaStorage then use
separate structural projections over that tree. This follows the robust part
of DaJet's `ConfigFileReader` approach while avoiding hard-coded byte offsets in
the tokenizer. A set of unrelated ad-hoc regular expressions was rejected
because nested collections and escaped quotes make them ambiguous.

### Implement raw DEFLATE in the crate

A bounded RFC 1951 decoder will support stored, fixed-Huffman, and
dynamic-Huffman blocks. This avoids a Cargo dependency and avoids shelling out
from the public library. The decoder enforces an output-size limit so corrupted
database blobs cannot grow memory without bound. Linking to a system zlib was
rejected because it would replace a Cargo dependency with a less explicit ABI
dependency.

### Separate authoritative and observational sources

DBNames determines GUID-to-number and alias mappings. Descriptors recursively
projected from bare-GUID Config resources determine human names; a resource can
contain both its owner descriptor and nested attribute descriptors.
SchemaStorage determines canonical physical schema declarations and references.
PostgreSQL catalogs only report what currently exists. Resolution never
promotes a catalog-name heuristic into an authoritative mapping.

The core `MetadataSnapshot` will therefore contain source-specific records plus
resolved objects. Missing Config or live-catalog data is reportable per object;
missing or invalid DBNames is fatal because no safe mapping exists.

### Keep GUIDs dependency-free

GUIDs are validated and stored as a canonical lowercase 36-character value in
a dedicated type rather than adding a UUID crate. Parsing checks the exact
8-4-4-4-12 hexadecimal layout. This representation is sufficient for lookup,
display, ordering, and equality without byte-order ambiguity.

### Acquire PostgreSQL data through psql in the CLI

`src/main.rs` will invoke `psql` directly with argument arrays, `-X`, `-w`,
unaligned output, and `PGOPTIONS=-c default_transaction_read_only=on`. SQL is
fixed by the executable; connection values are process arguments rather than
SQL interpolation. Authentication remains with `.pgpass`/`PGPASSFILE` or the
caller's environment, so passwords are neither parsed nor echoed by open-sdbl.

The library accepts byte resources and catalog rows and contains no process or
network I/O. A native PostgreSQL crate was rejected because it would add a
production dependency and duplicate the already installed client requested for
the target environment.

### Recase only catalog identifiers

PostgreSQL rows are normalized using a longest-token-first table of canonical
1C tokens. SchemaStorage and configuration resources retain their spelling.
This prevents prefix collisions such as `AccRg`/`Acc` and `SeqB`/`Seq` while
keeping case repair an explicit adapter-boundary operation.

### Render a stable tab-separated report

The initial CLI report uses a record discriminator and escaped tab-separated
fields for GUID, kind, name, physical name, field owner, and live status. It
emits both object and field rows. A hand-written JSON format was rejected
because a stable JSON contract deserves a serializer and versioning decision of
its own.

## Risks / Trade-offs

- **[Platform serialization evolves]** → Keep the generic parser tolerant of
  unknown atoms but strict about malformed structure; validate projections
  against real platform 8.3.27 resources and retain fixtures.
- **[Pure DEFLATE implementation is security-sensitive]** → Bound output,
  validate every bit read and back-reference, add fixed/dynamic/stored fixtures,
  and compare decoded real blobs with independently decoded evidence.
- **[psql text transport can be ambiguous]** → Request hexadecimal bytea and
  use separators only for fields whose platform grammar excludes those bytes;
  reject malformed rows.
- **[SchemaStorage layout contains undocumented variants]** → Preserve unknown
  subtrees and only project structures with verified signatures. Report
  unsupported declarations rather than guessing.
- **[Live database can be restructuring]** → Show declaration-versus-live
  mismatches; never rewrite one source to make it match another.
- **[Large configurations require many Config blobs]** → Query only bare-GUID,
  part-zero resources, recursively collect their descriptors, and resolve in
  memory once. Streaming can be introduced later without changing the model.

## Migration Plan

This is additive. Land the parser and model behind the new library module, then
enable the CLI command after unit and live read-only tests pass. Rollback is
removal of the new command and module; no information-base migration or data
repair is required because the workflow performs only SELECT operations.
