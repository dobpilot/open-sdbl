## ADDED Requirements

### Requirement: Stream PostgreSQL Config acquisition with bounded decoding
The PostgreSQL adapter SHALL consume bare-GUID, part-zero Config rows as an
asynchronous stream and decode resources through a bounded set of blocking CPU
jobs. Database row delivery and resource decoding SHALL be able to make
progress concurrently, completed descriptors SHALL retain query source order,
and decoder or database errors SHALL abort the read-only transaction. The
adapter SHALL NOT materialize the complete Config row set before decoding.

#### Scenario: Network and decoder overlap
- **WHEN** Config rows continue arriving while earlier resources are being
  decoded
- **THEN** the bounded pipeline polls database delivery and blocking decoder
  jobs concurrently up to its configured in-flight limit

#### Scenario: Decoder backpressure
- **WHEN** decoding is slower than row delivery
- **THEN** the adapter retains only the bounded in-flight resources and stops
  polling additional rows until capacity becomes available

#### Scenario: Stable descriptor order
- **WHEN** concurrently submitted resources finish out of order
- **THEN** their descriptors are appended in Config query source order

#### Scenario: CPU isolation
- **WHEN** DBNames, Config, SchemaStorage, or final resolution performs
  CPU-heavy work
- **THEN** that work executes outside Tokio asynchronous runtime workers

### Requirement: Query exact Config progress totals read-only
Before streaming Config, the PostgreSQL adapter SHALL obtain exact resource and
compressed-byte totals with a fixed SELECT-only query using the same row
predicate as the Config stream.

#### Scenario: Matching progress denominator
- **WHEN** the Config stream contains bare-GUID part-zero resources
- **THEN** progress totals count exactly those resources and their compressed
  `BinaryData` bytes
