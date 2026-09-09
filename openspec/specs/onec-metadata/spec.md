# onec-metadata Specification

## Purpose
Define deterministic, read-only resolution of logical 1C metadata names to the
physical PostgreSQL schema used by an information base.

## Requirements

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

### Requirement: Treat DBNames as the authoritative physical-name map
The library SHALL obtain GUID, alias, and numeric database identifiers from
`Params` where `FileName = 'DBNames'`, preserve multiple entries for one GUID,
and SHALL NOT infer a GUID or logical name from a physical table number.

#### Scenario: Main object mapping
- **WHEN** DBNames contains
  `{b56f25d2-72a9-4d80-8998-77ac3097c873,"Reference",2565}`
- **THEN** the resolved main physical table is `_Reference2565`

#### Scenario: Shared field and separator entries
- **WHEN** one GUID has `Fld` and `DataSeparationUse` or
  `DataSeparationHolder` entries
- **THEN** the field number resolves to that GUID and the field is marked as a
  data separator

#### Scenario: Missing authoritative map
- **WHEN** DBNames is absent, empty, compressed incorrectly, or contains no
  valid entries
- **THEN** resolution fails without scanning physical names as a fallback

### Requirement: Resolve supported tabular metadata kinds
The library SHALL map the DBNames aliases `Reference`, `Document`, `Enum`,
`InfoRg`, `AccumRg`, `AccRg`, `CRg`, `Chrc`, `CKinds`, `Acc`, `Const`, `Node`,
`BPr`, `Task`, and `Seq` to their distinct logical kinds and canonical physical
prefixes.

#### Scenario: Ambiguous prefixes
- **WHEN** accounting-register and chart-of-accounts objects are present
- **THEN** `AccRg` resolves to `_AccRg` and `Acc` resolves independently to
  `_Acc`

#### Scenario: Non-tabular descriptor
- **WHEN** a bare-GUID Config descriptor has no recognized tabular DBNames alias
- **THEN** its GUID and human name remain available without inventing a physical
  table

### Requirement: Obtain human names from bare-GUID Config descriptors
The library SHALL resolve metadata names, localized synonyms, and descriptor
markers from descriptors contained in assembled `Config` resources whose
`FileName` is a bare GUID. A resource MAY contain descriptors for its owner
and for nested metadata such as attributes. Resources named `<guid>.<part>`
SHALL NOT be used as the source of metadata names.

#### Scenario: Object and attribute descriptors
- **WHEN** a bare-GUID resource contains a nested descriptor `{1,0,<guid>},"КоррСчет",{2,"ru","Корр. счет"}`
- **THEN** the resolved name is `КоррСчет` and the Russian synonym is `Корр. счет`

#### Scenario: Additional resource slot
- **WHEN** only `<guid>.0` contains a matching string
- **THEN** that string is not accepted as the object's metadata name

### Requirement: Parse the authoritative physical schema
The library SHALL parse plaintext `SchemaStorage.CurrentSchema` for
`SchemaID = 0` into physical tables, columns, declared indexes, column type
tags, and reference targets. The parser SHALL preserve canonical 1C spelling
and SHALL NOT require SQL foreign keys. For an `R` declaration, an explicitly
empty target string SHALL be preserved and SHALL identify a universal
reference whose concrete type is stored in the physical `RTRef` member; it
SHALL remain distinguishable from a missing or malformed target.

#### Scenario: Reference column
- **WHEN** SchemaStorage contains a column definition with
  `{"R",...,"Reference35",...}`
- **THEN** the column exposes `Reference35` as its schema reference target

#### Scenario: Universal reference column
- **WHEN** SchemaStorage contains a valid `R` declaration with an explicitly
  empty target string
- **THEN** the parsed column retains that empty target as the universal-reference
  marker instead of reporting that the target is absent

#### Scenario: Stable schema snapshot
- **WHEN** both SchemaStorage and DBSchema exist
- **THEN** SchemaStorage remains the source used to compare with live indexes

### Requirement: Collapse physical columns into logical fields
The library SHALL collapse the physical suffix groups for references,
enumerations, value storage, and compound values into logical fields, remove a
leading underscore from the logical name, and exclude DBNames data-separation
fields when comparing index keys.

#### Scenario: Reference pair
- **WHEN** `_Fld12_TRef` and `_Fld12_RRRef` occur in one table
- **THEN** they resolve to one logical field named `Fld12`

#### Scenario: Recorder reference pair
- **WHEN** `_RecorderTRef` and `_RecorderRRef` occur in one table
- **THEN** they resolve to one logical field named `Recorder`

#### Scenario: Compound value
- **WHEN** columns share a base and use `_TYPE`, `_L`, `_N`, `_T`, `_S`,
  `_RTRef`, and `_RRRef` suffixes
- **THEN** they resolve to one compound logical field with all physical members

### Requirement: Normalize PostgreSQL catalog identifiers at the boundary
The PostgreSQL adapter SHALL compare lowercase catalog identifiers with
canonical SchemaStorage spelling using longest-token-first recasing and SHALL
leave parsed DBNames, Config, and SchemaStorage text unchanged.

#### Scenario: Longest prefix wins
- **WHEN** PostgreSQL reports `_accrg3942` or `_seqb10`
- **THEN** recasing produces `_AccRg3942` or `_SeqB10`, not `_Accrg3942` or
  `_Seqb10`

#### Scenario: Compound suffixes
- **WHEN** PostgreSQL reports `_fld12_rrref` and `_fld12_type`
- **THEN** recasing produces `_Fld12_RRRef` and `_Fld12_TYPE`

### Requirement: Verify against the live PostgreSQL catalog read-only
The `open-sdbl-cli` package SHALL provide `open-sdbl metadata postgres` and use
`tokio-postgres` to execute fixed SELECT-only queries in one explicit read-only
`READ COMMITTED` transaction. It SHALL detect the storage layout, then read
DBNames, bare-GUID Config descriptors, SchemaStorage, tables, columns, and
indexes and SHALL report resolved and missing physical objects without
extracting, guessing, or printing 1C or PostgreSQL user passwords.

#### Scenario: Resolved information base
- **WHEN** valid connection options identify a PostgreSQL 1C information base
- **THEN** the command prints each resolved GUID, kind, human name, canonical physical name, owner table for fields, and live-catalog status

#### Scenario: PostgreSQL authentication
- **WHEN** PostgreSQL requires a password
- **THEN** the CLI obtains it from `PGPASSWORD`, an explicit `PGPASSFILE`, or the default `.pgpass` file and does not accept or print it as a command-line argument

#### Scenario: Read-only enforcement
- **WHEN** the CLI acquires live metadata
- **THEN** every metadata query executes inside a read-only `READ COMMITTED` transaction and no mutating SQL is executed

#### Scenario: No PostgreSQL subprocess
- **WHEN** the CLI connects to PostgreSQL
- **THEN** it uses the asynchronous driver directly and does not require or spawn the `psql` executable

### Requirement: Provide indexed metadata identity lookup
The resolved metadata snapshot SHALL expose GUID-backed object and attribute
IDs, stable numeric standard-field IDs, and indexed lookup by object GUID,
normalized kind/name, owner/name, and database reference type number. Expected
lookup time SHALL be O(1), excluding normalization and result cloning.
Ambiguity and the absence of a Config GUID for a standard field SHALL be typed
outcomes.

#### Scenario: Object GUID by kind and name
- **WHEN** an application looks up a unique object by supported kind and
  logical name
- **THEN** the snapshot returns its real 16-byte metadata GUID

#### Scenario: Attribute GUID by owner and name
- **WHEN** an application looks up a custom attribute by owner GUID and logical
  name
- **THEN** the snapshot returns the attribute's real 16-byte metadata GUID

#### Scenario: Standard field lookup
- **WHEN** an application looks up `Код`, `Наименование`, or another standard
  field
- **THEN** field lookup returns its numeric standard-field ID and strict
  attribute-GUID lookup reports that the field has no metadata GUID

#### Scenario: Reference type lookup
- **WHEN** a physical RTRef value identifies a DBNames table number
- **THEN** indexed lookup returns the corresponding object GUID without
  guessing from a physical table name

### Requirement: Preserve information-register field purpose
The metadata decoder SHALL classify custom information-register fields as
dimensions, resources, or attributes from the enclosing Config collection GUID
and SHALL expose that purpose on the resolved field. It SHALL NOT infer field
purpose from physical names or index layouts.

#### Scenario: Information-register dimension
- **WHEN** a descriptor is nested in the Config collection identified by
  `13134203-f60b-11d5-a3c7-0050bae0a776`
- **THEN** its resolved custom field is classified as an information-register
  dimension

#### Scenario: Unknown collection
- **WHEN** a descriptor has no recognized information-register collection
  ancestor
- **THEN** its resolved field purpose remains unknown rather than guessed

### Requirement: Preserve accumulation-register field purpose
The metadata decoder SHALL classify custom accumulation-register fields as
dimensions, resources, or attributes from their enclosing Config collection
GUID and expose that purpose on resolved fields. It SHALL NOT infer these roles
from physical names or index layouts.

#### Scenario: Accumulation-register roles
- **WHEN** descriptors occur under the recognized accumulation-register
  dimension, resource, and attribute collection GUIDs
- **THEN** each resolved field carries the corresponding typed purpose

#### Scenario: Unknown accumulation collection
- **WHEN** no recognized collection encloses a descriptor
- **THEN** its purpose remains unknown instead of being guessed

### Requirement: Decode DEFLATE Huffman symbols with bounded lookup cost
The metadata decoder SHALL resolve each fixed or dynamic DEFLATE Huffman symbol
with lookup work bounded by the RFC 1951 maximum code width rather than by the
number of entries in the Huffman tree. Lookup storage SHALL remain bounded by
the 15-bit DEFLATE code limit. The optimization SHALL preserve decoded bytes,
output-size limits, and rejection of empty, oversubscribed, incomplete, or
truncated invalid streams.

#### Scenario: Large compressed Config resource
- **WHEN** a Config resource contains many symbols encoded with a populated
  dynamic Huffman tree
- **THEN** decoding does not scan the tree entries for every output symbol

#### Scenario: Short code at physical input end
- **WHEN** the final valid symbol needs fewer bits than the tree maximum and is
  followed only by DEFLATE byte padding
- **THEN** lookup consumes only the symbol's actual code length and succeeds

#### Scenario: Invalid incomplete-tree prefix
- **WHEN** input bits address an unassigned prefix in an incomplete Huffman tree
- **THEN** decoding returns the existing invalid-Huffman-code diagnostic

### Requirement: Project authoritative DBNames while parsing
The metadata decoder SHALL validate the complete brace-serialized DBNames
resource and collect valid `{GUID,"Alias",Number}` entries in source order
without requiring a complete generic value tree. Memory retained during parsing
SHALL be bounded by accepted entries, decoded input, and nesting depth rather
than by every serialized scalar and list. The public generic value parser SHALL
remain available and compatible.

#### Scenario: Large nested DBNames map
- **WHEN** a DBNames resource contains many nested lists and irrelevant scalar
  values around valid entries
- **THEN** the decoder collects the valid entries without retaining generic
  nodes for the irrelevant values

#### Scenario: Malformed irrelevant branch
- **WHEN** any nested DBNames branch has an unterminated list or string even if
  it could not project to an entry
- **THEN** the complete DBNames resource is rejected with a positional
  diagnostic

#### Scenario: Entry compatibility
- **WHEN** a nested list has exactly the GUID, quoted alias, and positive number
  shape accepted by the existing projection
- **THEN** the streaming projection returns the same canonical entry in the
  same source order

### Requirement: Scan brace serialization without repeated character decoding
After validating UTF-8, the generic metadata value parser SHALL recognize ASCII
structural delimiters and contiguous string spans without repeatedly decoding
each character. It SHALL preserve the public owned `Value` hierarchy, Unicode
text and whitespace, doubled-quote unescaping, byte-position diagnostics, and
malformed-input rejection.

#### Scenario: Long localized string
- **WHEN** a quoted Config value contains long multibyte text and doubled quotes
- **THEN** parsing returns the identical unescaped string without per-character
  structural checks

#### Scenario: Unicode whitespace
- **WHEN** valid non-ASCII whitespace surrounds a serialized value
- **THEN** the parser accepts it with the same semantics as ASCII whitespace

#### Scenario: Malformed UTF-8 or quoting
- **WHEN** input is byte-invalid UTF-8 or a quoted value is unterminated
- **THEN** parsing returns the same class of positional diagnostic

### Requirement: Project Config descriptors while parsing
The metadata decoder SHALL validate each bare-GUID Config resource and project
owner and nested descriptors without retaining a complete generic value tree.
It SHALL preserve descriptor source order, marker, object GUID, name, synonyms,
optional comment, and inherited field purpose. Intermediate retained state
SHALL be bounded by decoded input, output descriptors, nesting depth, a
four-candidate window per active list, and scalar-only lists required by a
matching descriptor.

#### Scenario: Owner and nested descriptors
- **WHEN** one Config resource contains an owner descriptor and nested attribute
  descriptors among unrelated complex branches
- **THEN** streaming projection returns the same descriptors in source order
  without retaining unrelated branches

#### Scenario: Inherited collection purpose
- **WHEN** a recognized register collection contains a nested field descriptor
- **THEN** the field receives the same inherited dimension, resource, or
  attribute purpose as the generic recursive projection

#### Scenario: Descriptor presentation fields
- **WHEN** a matching descriptor has localized synonym pairs and an immediately
  following nonempty comment
- **THEN** both fields are preserved exactly

#### Scenario: Malformed Config branch
- **WHEN** any branch of a bare-GUID Config resource is malformed
- **THEN** parsing fails positionally even when that branch cannot match a
  descriptor

### Requirement: Resolve live metadata from precomputed membership indexes
The metadata resolver SHALL derive SchemaStorage field ownership, live field
presence, and live table identity with lookup structures built in bounded
passes over their respective inputs rather than rescanning all tables and
columns for every DBNames object or field. It SHALL preserve observable
resolution results and source order.

#### Scenario: Schema field ownership
- **WHEN** a canonical `Fld<number>` column or one of its compound members is
  declared by multiple SchemaStorage tables
- **THEN** the resolved field lists each owning table once in SchemaStorage
  source order

#### Scenario: Live compound field presence
- **WHEN** the live catalog contains an exact `_fld<number>` column or a
  compound member separated by `_`
- **THEN** the corresponding resolved field is live after canonical recasing

#### Scenario: Similar numeric prefix
- **WHEN** the catalog contains `_fld123` but DBNames contains only `Fld12`
- **THEN** the resolver does not treat the longer numeric field as a match

#### Scenario: Live table identity
- **WHEN** a DBNames object has a corresponding lowercase PostgreSQL table
- **THEN** object live-state, allowed-length inference, and standard-field
  indexing match the prior case-insensitive resolution

### Requirement: Build an indexed queryable-field catalog
The query layer SHALL provide a snapshot-scoped catalog of queryable fields for
the snapshot's current public object, field, schema, and live-table vectors. A
catalog build SHALL index custom fields by normalized owner table and numeric
DBNames field number rather than rescan all fields for every object. The
snapshot SHALL NOT retain indexes that become stale when callers modify those
public vectors.

#### Scenario: Unique owned custom field
- **WHEN** one owner declares `Fld<number>` and its Config descriptor has a
  human name
- **THEN** catalog projection obtains that name through indexed owner and
  number identity

#### Scenario: Caller-modified snapshot
- **WHEN** a caller changes public fields, schema, or live tables after
  resolution and then rebuilds the catalog
- **THEN** the catalog reflects current vectors and preserves the existing
  source-order first-match semantics

### Requirement: Stream PostgreSQL Config acquisition with bounded decoding
The PostgreSQL adapter SHALL consume bare-GUID Config rows as an asynchronous
stream, assemble multi-part resources in row order, and decode resources
through a bounded set of blocking CPU jobs. Database row delivery and resource
decoding SHALL be able to make progress concurrently. Completed descriptors
SHALL be returned in ascending Config filename order and retain source order
within each resource regardless of PostgreSQL row-delivery order. Decoder or
database errors SHALL abort the read-only transaction. The adapter SHALL NOT
materialize the complete compressed Config row set before decoding.

#### Scenario: Network and decoder overlap
- **WHEN** Config rows continue arriving while earlier resources are being decoded
- **THEN** the bounded pipeline polls database delivery and blocking decoder jobs concurrently up to its configured in-flight limit

#### Scenario: Decoder backpressure
- **WHEN** decoding is slower than row delivery
- **THEN** the adapter retains only the bounded in-flight compressed resources and stops polling additional rows until capacity becomes available

#### Scenario: Stable descriptor order
- **WHEN** PostgreSQL delivers Config resources in an order different from ascending filename order
- **THEN** their descriptors are returned in ascending filename order while retaining descriptor source order within each resource

#### Scenario: CPU isolation
- **WHEN** DBNames, Config, SchemaStorage, or final resolution performs CPU-heavy work
- **THEN** that work runs on Tokio's blocking pool rather than a runtime worker

### Requirement: Query exact Config progress totals read-only
Before streaming Config, the PostgreSQL adapter SHALL obtain exact resource and
compressed-byte totals with a fixed SELECT-only query using the same row
predicate as the Config stream, counting distinct resource names and the bytes
of every part.

#### Scenario: Matching progress denominator
- **WHEN** the Config stream contains bare-GUID resources split into parts
- **THEN** progress totals count each resource once and all of its compressed `BinaryData` bytes

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

### Requirement: Keep snapshot lookups internally consistent
A resolved metadata snapshot SHALL NOT expose mutable access to the
collections its internal lookup index refers to. Identity-based lookups
SHALL either return the object originally indexed or a typed error —
never a panic and never a different object.

#### Scenario: Snapshot collections are read-only
- **WHEN** an application holds a resolved snapshot
- **THEN** it can read objects, fields, values, and live tables through
  accessors but cannot remove or reorder entries underneath the lookup
  index

#### Scenario: Lookup after resolution
- **WHEN** any identity lookup is performed on a resolved snapshot
- **THEN** the result is the originally resolved entity or a typed
  lookup error

### Requirement: Project every declared inline table kind
The SchemaStorage projection SHALL recognize all inline table kinds
declared by the platform — tabular sections, accounting extra
dimensions, calculation-kind dependency tables, change-registration
tables, and recalculation tables — including their owner linkage and
synthesized owner-reference columns. Inline declarations of an unknown
kind SHALL still be isolated and reported, never silently dropped.

#### Scenario: Extra-dimension declaration
- **WHEN** a chart of accounts declares extra-dimension tables
- **THEN** the projection yields tables linked to their owning chart
  with their key columns intact

#### Scenario: Unknown inline kind
- **WHEN** a declaration uses an inline kind the library does not know
- **THEN** the table is recorded as a schema anomaly and the containing
  object remains resolved

### Requirement: Resolve platform service tables as typed metadata
Metadata resolution SHALL resolve change-registration, recalculation,
calculation-kind dependency, and extra-dimension tables to typed
metadata objects owned by their registered object, chart, or register,
using DBNames as the naming authority.

#### Scenario: Change-registration ownership
- **WHEN** a resolved base contains change-registration tables
- **THEN** each resolves to a typed object linked to the object whose
  changes it registers, with queryable node and key columns

#### Scenario: Service mapping mismatch
- **WHEN** a non-zero service DBNames entry has no matching
  SchemaStorage declaration by alias and number
- **THEN** resolution records a typed finding instead of silently
  dropping the entry

### Requirement: Merge extension declarations into owning objects
Given the extension side of Config and DBNames, resolution SHALL merge
extension-added attributes into the owning object's queryable fields,
mapped to the extension's physical columns, and SHALL record their
extension origin. Extension tables that mirror a declared base object
SHALL count as declared.

#### Scenario: Extension-added attribute
- **WHEN** an extension adds an attribute to a configuration object
- **THEN** the resolved object exposes that attribute as a queryable
  field mapped to the extension table's column

#### Scenario: Extension field-number collision
- **WHEN** an extension maps an existing base `Fld` number to a
  different GUID
- **THEN** the base mapping is retained, the conflicting extension
  field is rejected, and resolution records a typed finding

#### Scenario: Report silence for supported families
- **WHEN** a base with extensions and service tables is resolved with
  extension inputs supplied
- **THEN** the resolution report lists only genuinely unknown tables

### Requirement: Parse reference dumps without spurious findings
Decoding and resolution SHALL be exercised against whole `DBNames` and
`SchemaStorage` dumps captured from reference bases, and SHALL neither
fail nor emit findings that do not correspond to a real mismatch.

#### Scenario: Shared and zero GUID entries
- **WHEN** a real `DBNames` resource contains many entries that share an
  owner GUID and many entries carrying the all-zero GUID
- **THEN** decoding succeeds, the all-zero entries resolve to no object,
  and no duplicate-GUID finding is produced for either group

#### Scenario: Complete column-tag coverage
- **WHEN** a real `SchemaStorage` resource is projected
- **THEN** every declared column type tag is recognized and no
  unknown-column-tag or invalid-declaration finding is produced

#### Scenario: Cross-provider equivalence
- **WHEN** the same logical configuration is resolved from a PostgreSQL
  dump and from an MSSQL dump
- **THEN** shared objects resolve to equivalent canonical names,
  physical names, and field sets

### Requirement: Detect the storage layout before acquisition
Before reading DBNames, the application adapters SHALL determine with one
SELECT-only catalog query that cannot fail whether `Params` and `Config`
carry a `PartNo` column, whether `ConfigCAS` and `_ExtensionsRestruct` exist,
and whether `SchemaStorage` exists. The core SHALL expose the result as a
`StorageLayout` and SHALL select the matching query variant for every
acquisition statement. Absent extension tables SHALL be treated as "no
extensions" without issuing their queries; an absent `SchemaStorage` SHALL
produce a typed error before any further statement.

#### Scenario: Platform 8.2 base
- **WHEN** `Config` has no `PartNo` column and `ConfigCAS` does not exist
- **THEN** DBNames and Config are read with the legacy variants, extension
  readers return empty lists, and metadata resolves normally

#### Scenario: Modern base
- **WHEN** `Params`, `Config`, and `ConfigCAS` carry `PartNo`
- **THEN** the multi-part variants are used and every statement remains
  SELECT-only

#### Scenario: Base without SchemaStorage
- **WHEN** the catalog has no `SchemaStorage` table
- **THEN** acquisition fails with a typed metadata error naming the missing
  table instead of a database error

### Requirement: Assemble multi-part resources
When a file table carries `PartNo`, the adapters SHALL read every part of a
resource ordered by name and part number and SHALL concatenate `BinaryData`
in ascending `PartNo` before decoding. Parts SHALL be numbered contiguously
from zero; a gap or a sequence that does not start at zero SHALL be a data
error naming the resource. Without `PartNo` a resource SHALL be exactly one
row.

#### Scenario: Three-part descriptor
- **WHEN** a bare-GUID Config resource is stored as parts 0, 1, and 2
- **THEN** the decoder receives one compressed payload equal to the
  concatenation of the three parts in order

#### Scenario: Missing part
- **WHEN** a resource has parts 0 and 2 but no part 1
- **THEN** acquisition fails with a data error naming the resource and the
  missing part

### Requirement: Acquire authoritative 1C metadata from Microsoft SQL Server
The application SHALL read DBNames from `Params`, assembled GUID-named Config
resources from `Config`, the current schema from `SchemaStorage`, and live
tables, columns, and ordered index keys from the SQL Server `dbo` catalog using
fixed SELECT-only queries supplied by the core library. Returned resources
SHALL be decoded and resolved by the same deterministic core APIs as PostgreSQL.

#### Scenario: SQL Server DBNames and Config
- **WHEN** `metadata mssql` connects to a supported 1C SQL Server database
- **THEN** it reads binary DBNames and Config resources without converting or
  truncating their bytes

#### Scenario: SQL Server live catalog
- **WHEN** the database contains physical 1C tables and indexes in `dbo`
- **THEN** metadata resolution receives their case-preserved names, normalized
  type declarations, uniqueness flags, and key columns in ordinal order

#### Scenario: Acquisition safety
- **WHEN** all MSSQL metadata query definitions are inspected
- **THEN** every statement is SELECT-only and scoped to the connected database

### Requirement: Read the SQL Server 1C year offset
The MSSQL application provider SHALL read `dbo._YearOffset.Offset`, accept the
1C values 0 and 2000, and pass the value to MSSQL query compilation.

#### Scenario: Offset 2000 infobase
- **WHEN** `_YearOffset.Offset` is 2000
- **THEN** projected physical datetime values are shifted back by 2000 years
  and logical date literals used for database filtering are shifted forward by
  2000 years

### Requirement: Resolve extension attributes from the restructure resource
Given the extension restructure resource
(`_ExtensionsRestruct._restructData`), resolution SHALL map each
extension-added attribute to its physical extension column, expose it as
a queryable field of the owning object with its logical name and
extension origin, and record its type. Malformed restructure records
SHALL be reported, not panicked on, and SHALL NOT drop the surrounding
object.

#### Scenario: Extension attribute becomes queryable
- **WHEN** a base is resolved together with an extension whose
  restructure declares an attribute on an existing object
- **THEN** the owning object exposes that attribute as a queryable field
  mapped to the extension table's physical column, and a query selecting
  it compiles on both dialects instead of failing with a missing-field
  diagnostic

#### Scenario: Wildcard includes extension attributes
- **WHEN** a query selects all fields of an extended object
- **THEN** the compiled projection includes the extension attribute
  columns from the extension table

#### Scenario: Malformed restructure record
- **WHEN** an extension restructure record cannot be interpreted
- **THEN** resolution records a typed finding and the object's remaining
  fields still resolve

### Requirement: Acquire predefined-value metadata

The application adapters SHALL load assembled bare-GUID Config resources and
assembled `<guid>.1c` resources. The core SHALL decode only verified `.1c`
predefined-value rows and associate every value with the catalog GUID encoded
in its file name. Other Config suffixes SHALL remain excluded.

#### Scenario: Catalog predefined values
- **WHEN** `<catalog-guid>.1c` contains verified predefined rows
- **THEN** each exact symbolic name and stable GUID is associated with that
  catalog without reading catalog business rows

#### Scenario: Unrelated suffix resource
- **WHEN** a Config file has a suffix other than `.1c`
- **THEN** predefined-value decoding returns no values and does not interpret
  the payload as metadata

### Requirement: Resolve predefined values deterministically

The metadata snapshot SHALL index exact normalized predefined names by owning
object. Enumeration child descriptors and catalog `.1c` rows SHALL share the
same lookup contract. Missing and ambiguous values SHALL remain distinct typed
outcomes.

#### Scenario: Enumeration value
- **WHEN** a child descriptor belongs to a resolved enumeration resource
- **THEN** its metadata GUID and name are indexed under that enumeration

#### Scenario: Ambiguous value
- **WHEN** two values normalize to the same name under one owner
- **THEN** lookup reports ambiguity instead of selecting one by source order

### Requirement: Convert metadata GUIDs to 1C storage bytes

The core SHALL convert canonical UUID fields `a-b-c-d-e` to physical 1C byte
order `d + e + c + b + a` without changing byte order inside a field.

#### Scenario: Known enumeration GUID
- **WHEN** GUID `d2f8bde9-fadd-4be8-9022-249e3a1ac4b9` is converted
- **THEN** the bytes are `9022249e3a1ac4b94be8faddd2f8bde9`

### Requirement: Read the live PostgreSQL catalog on servers before 9.4
The PostgreSQL adapter SHALL read `server_version_num` inside the read-only
transaction and SHALL use a catalog statement without `LATERAL` or
`WITH ORDINALITY` when the server is older than 9.4. Both statements SHALL
return the same `(tag, table, name, detail, columns)` rows with index columns
in key order.

#### Scenario: PostgreSQL 9.2
- **WHEN** the server reports `server_version_num` below 90400
- **THEN** the adapter lists tables, columns, and ordered index keys through
  the legacy statement

#### Scenario: Modern server
- **WHEN** the server reports 9.4 or newer
- **THEN** the adapter uses the `LATERAL` statement and the resolved snapshot
  is identical to the legacy statement's result
