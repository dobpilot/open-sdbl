## ADDED Requirements

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

## MODIFIED Requirements

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
