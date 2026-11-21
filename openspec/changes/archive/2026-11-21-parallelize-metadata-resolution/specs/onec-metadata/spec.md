## ADDED Requirements

### Requirement: Resolve independent metadata fields concurrently
Complete snapshot finalization SHALL resolve independent DBNames field entries concurrently on bounded scoped worker threads when the input is large enough to benefit. Worker count SHALL NOT exceed available parallelism, and small inputs SHALL retain a synchronous path. The existing outer Tokio blocking boundary SHALL remain in force.

#### Scenario: UNF-sized field inventory
- **WHEN** a snapshot with thousands of fields is finalized on a multi-CPU host
- **THEN** independent entries are resolved in ordered chunks on multiple workers without running CPU-heavy work on Tokio runtime workers
- **AND** resolution report, metadata-index and fingerprint jobs then run concurrently without changing the fingerprint byte sequence

### Requirement: Preserve deterministic output of parallel field resolution
Parallel field resolution SHALL produce the same fields, separator findings, resolution report and snapshot fingerprint as sequential resolution. Fields SHALL retain ascending DBNames entry order, descriptors SHALL retain their existing source order and worker completion order SHALL NOT affect output.

#### Scenario: Slow and fast workers complete out of order
- **WHEN** independent field chunks finish in an order different from DBNames entry order
- **THEN** the parent joins results in input order and the snapshot is byte-for-byte equivalent to sequential field resolution

#### Scenario: Small acquisition
- **WHEN** a selected or otherwise small snapshot resolves fewer entries than the parallel chunk threshold
- **THEN** it uses the synchronous path without spawning scoped workers
