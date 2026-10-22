## ADDED Requirements

### Requirement: Bootstrap configuration without resolving metadata
Asynchronous and blocking database sessions SHALL provide bootstrap acquisition containing storage layout, root and caller-selected configuration descriptor resources, and the raw extension catalog. Bootstrap SHALL NOT read other Config resources, ConfigCAS bodies, DBNames, SchemaStorage or live-table catalogs, and SHALL NOT construct a MetadataSnapshot. Existing selector callbacks SHALL determine the descriptor from root and inspect the descriptor.

#### Scenario: Discover a configuration
- **WHEN** bootstrap succeeds
- **THEN** it returns exactly root, descriptor, layout and extension catalog without a full metadata read

### Requirement: Read explicitly addressed configuration resources
Both session forms SHALL read requested Config names and typed ConfigCAS content keys using SQL predicates before resource collection. Repeated requests SHALL be deduplicated in first-request order. Missing resources and noncontiguous parts SHALL fail the complete call rather than return partial success. Empty requests SHALL NOT query resource tables. Existing compressed-byte and resource-count limits SHALL bound each collection, including accumulation of multipart resources before assembly completes.

#### Scenario: Read two resources and then one more
- **WHEN** a session reads two named resources and later another resource
- **THEN** each query addresses only that call's names and reuses the known storage layout

#### Scenario: Missing part or resource
- **WHEN** any requested resource is missing or its part sequence skips a part
- **THEN** the call fails and does not return a shortened result

#### Scenario: Empty or repeated requests
- **WHEN** a request is empty or contains repeated names
- **THEN** empty requests read no resource table and repeated names produce one result per distinct name in request order

#### Scenario: Limits before assembly completes
- **WHEN** incoming parts exceed the configured compressed-byte limit
- **THEN** the read fails before appending those parts to an assembled resource

### Requirement: Preserve session lifecycle for indexed reads
Indexed operations SHALL reuse the owning connection, cache detected layout, use existing read-only transaction cleanup, limits and timeouts, and preserve SQL Server poisoning and blocking closed-session/async-context diagnostics. Reads SHALL NOT promise an atomic snapshot across calls or compute a fingerprint per session.

#### Scenario: Closed blocking session
- **WHEN** an indexed operation is called after close, including inside an asynchronous runtime
- **THEN** it reports SessionClosed before checking asynchronous context

#### Scenario: Repeated addressed reads
- **WHEN** bootstrap is followed by addressed reads
- **THEN** they use the same connection without probing layout again
