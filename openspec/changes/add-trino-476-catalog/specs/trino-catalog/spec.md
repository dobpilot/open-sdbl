## ADDED Requirements

### Requirement: Expose resolved 1C objects as deterministic Trino metadata
The integration SHALL expose every queryable live `MetadataSnapshot` object in
the Russian schema corresponding to its actual metadata kind and SHALL expose
logical Config field names as columns. Unicode spelling SHALL be preserved.
Missing names, conflicts, unsupported types, and non-live objects SHALL be
reported explicitly and SHALL NOT be silently discarded.

#### Scenario: Catalog discovery
- **WHEN** Trino requests schemas, tables, or columns
- **THEN** the connector returns deterministic names derived from the current
  immutable metadata generation

#### Scenario: Duplicate logical object name
- **WHEN** two live objects of one kind have the same case-folded Config name
- **THEN** both receive stable disambiguated names and the conflict is reported
  as a metadata issue

### Requirement: Map 1C storage types explicitly
The integration SHALL map representable boolean, integer, bigint, decimal,
string, date, timestamp, UUID/reference, binary, and compound values to explicit
Trino types without coercing all columns to varchar. Any lossy or encoded
representation SHALL be documented and tested.

#### Scenario: Exact numeric field
- **WHEN** a live 1C field uses PostgreSQL numeric storage with known precision
  and scale
- **THEN** the column is exposed as a compatible Trino decimal or integer type

#### Scenario: Compound field
- **WHEN** a logical 1C value consists of multiple physical members and has no
  exact scalar Trino representation
- **THEN** all members are exposed through a documented JSON or reference
  encoding instead of being silently dropped

### Requirement: Execute read-only scans with remote pushdown
The Rust service SHALL execute only validated SELECT scans in explicit
read-only PostgreSQL transactions. It SHALL project only requested physical
members, bind all literal values as parameters, and apply accepted predicates
and limits in PostgreSQL.

#### Scenario: Filtered limited projection
- **WHEN** Trino requests selected columns with a supported predicate and limit
- **THEN** generated PostgreSQL selects only those members and contains the
  bound predicate and limit before rows are streamed to Trino

#### Scenario: Unsupported predicate
- **WHEN** a domain cannot be translated losslessly
- **THEN** the Java connector leaves that domain for Trino evaluation and does
  not claim it was pushed

#### Scenario: Write attempt
- **WHEN** a user requests INSERT, UPDATE, DELETE, CREATE, or DROP through the
  catalog
- **THEN** the connector reports the operation as unsupported and sends no
  mutating request to PostgreSQL

### Requirement: Cache metadata without blocking every query
The Rust service SHALL publish thread-safe immutable `MetadataSnapshot`
generations with a configurable TTL. A refresh SHALL not block readers when a
valid previous generation exists, and a failed refresh SHALL retain that valid
generation while reporting the error.

#### Scenario: Concurrent reads after expiry
- **WHEN** multiple scans arrive after the metadata TTL expires
- **THEN** at most one refresh reconstructs metadata while scans continue with
  the last valid generation

### Requirement: Bound and observe the integration service
The service SHALL provide health and readiness endpoints, structured redacted
logs, lightweight query/metadata metrics, connection-pool bounds, query and
statement timeouts, and explicit result safeguards that never silently change
SQL semantics.

#### Scenario: Debug query record
- **WHEN** DEBUG logging is enabled for a completed scan
- **THEN** logs identify the logical object, selected fields, accepted
  predicates, parameterized SQL, duration, and returned rows without secrets

#### Scenario: Metadata not ready
- **WHEN** no valid metadata generation has loaded
- **THEN** readiness fails while liveness remains available

### Requirement: Target upstream Trino SPI 476
The Java connector SHALL depend on `io.trino:trino-spi:476`, use upstream SPI
interfaces only, and delegate all SDBL/1C metadata and PostgreSQL logic to the
Rust service. It SHALL use one actual split until a validated partitioning
strategy exists.

#### Scenario: Trino 476 acceptance query
- **WHEN** stock Trino 476 loads the plugin and runs discovery, describe, a
  limited projection, and a supported filtered limited projection
- **THEN** the queries succeed and the Rust debug record demonstrates remote
  projection, predicate, and limit application
