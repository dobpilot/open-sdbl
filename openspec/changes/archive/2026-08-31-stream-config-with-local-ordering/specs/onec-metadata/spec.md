## MODIFIED Requirements

### Requirement: Stream PostgreSQL Config acquisition with bounded decoding
The PostgreSQL adapter SHALL consume bare-GUID, part-zero Config rows as an
asynchronous stream and decode resources through a bounded set of blocking CPU
jobs. Database row delivery and resource decoding SHALL be able to make
progress concurrently. Completed descriptors SHALL be returned in ascending
Config filename order and retain source order within each resource regardless
of PostgreSQL row-delivery order. Decoder or database errors SHALL abort the
read-only transaction. The adapter SHALL NOT materialize the complete
compressed Config row set before decoding.

#### Scenario: Network and decoder overlap
- **WHEN** Config rows continue arriving while earlier resources are being
  decoded
- **THEN** the bounded pipeline polls database delivery and blocking decoder
  jobs concurrently up to its configured in-flight limit

#### Scenario: Decoder backpressure
- **WHEN** decoding is slower than row delivery
- **THEN** the adapter retains only the bounded in-flight compressed resources
  and stops polling additional rows until capacity becomes available

#### Scenario: Stable descriptor order
- **WHEN** PostgreSQL delivers Config resources in an order different from
  ascending filename order
- **THEN** their descriptors are returned in ascending filename order while
  retaining descriptor source order within each resource

#### Scenario: CPU isolation
- **WHEN** DBNames, Config, SchemaStorage, or final resolution performs
  CPU-heavy work
- **THEN** that work runs on Tokio's blocking pool rather than a runtime worker
