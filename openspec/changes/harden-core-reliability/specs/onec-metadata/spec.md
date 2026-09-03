## MODIFIED Requirements

### Requirement: Decode platform metadata resources
The library SHALL decode raw-DEFLATE `Params` and `Config` resources, accept
UTF-8 data with or without a byte-order mark, and parse the nested 1C
brace-serialized value format without changing string contents. Decoding
SHALL be total: bounded output size, bounded nesting depth, and bounded
per-block decoder work, so no malformed or adversarial resource can abort
the process or exhaust memory. The decoder SHALL accept RFC 1951 dynamic
blocks whose distance tree is empty when the block contains no matches.

#### Scenario: Compressed descriptor with escaped text
- **WHEN** a raw-DEFLATE configuration resource contains a BOM, nested
  lists, and a quoted string with doubled quotes
- **THEN** the library returns the original logical string and value
  hierarchy

#### Scenario: Malformed resource
- **WHEN** compressed data or brace serialization is truncated or malformed
- **THEN** the library returns a diagnostic instead of partial trusted
  metadata

#### Scenario: Pathologically nested serialization
- **WHEN** a decoded resource contains brace nesting beyond the documented
  depth limit
- **THEN** parsing returns a positional diagnostic instead of overflowing
  the stack

#### Scenario: Literal-only dynamic DEFLATE block
- **WHEN** a resource was compressed with a dynamic block declaring an
  empty distance tree and no matches
- **THEN** the library inflates it successfully

## ADDED Requirements

### Requirement: Resolve non-ASCII catalog identifiers without failure
Identifier recasing and live-catalog indexing SHALL accept identifiers
containing non-ASCII characters, passing them through without panicking and
without corrupting their encoding.

#### Scenario: Cyrillic user table in the live catalog
- **WHEN** the live catalog rows include a table or column whose name
  contains Cyrillic characters
- **THEN** metadata resolution completes and the identifier round-trips
  byte-for-byte

### Requirement: Report resolution mismatches
Metadata resolution SHALL return a structured report of mismatches between
DBNames, Config, SchemaStorage, and the live catalog — including missing
descriptors, declared objects without live tables, unknown column type
tags, and duplicate identifiers — instead of silently degrading affected
objects.

#### Scenario: Declared object without a live table
- **WHEN** SchemaStorage declares a table that the live catalog does not
  contain
- **THEN** the snapshot marks the object as not live and the resolution
  report identifies the object and the missing table

### Requirement: Preserve tables with unknown column type tags
When SchemaStorage contains a column whose type tag is not recognized, the
library SHALL keep the table and its recognized columns queryable and SHALL
record the unknown tag in the resolution report rather than dropping the
table. Malformed child counts, entries, and reference declarations SHALL
likewise be isolated and reported without discarding the containing table.

#### Scenario: Novel platform column tag
- **WHEN** one column of a declared table uses a type tag introduced by a
  newer platform version
- **THEN** the table remains declared, its other columns resolve normally,
  and the report names the unknown tag

#### Scenario: Malformed child declaration
- **WHEN** a recognized table contains a nonnumeric child count or a reference
  column without a target
- **THEN** the table remains available and the resolution report identifies
  the malformed declaration

### Requirement: Categorize metadata decoding errors
Metadata decoding errors SHALL expose a machine-readable kind and document
the unit of their offset (bit offset for DEFLATE failures, byte offset for
serialization parsers).

#### Scenario: Distinguishing decode failures
- **WHEN** one input fails DEFLATE decoding and another fails
  brace-serialization parsing
- **THEN** the two errors expose distinct kinds and interpretable offsets
  without message-text comparison
