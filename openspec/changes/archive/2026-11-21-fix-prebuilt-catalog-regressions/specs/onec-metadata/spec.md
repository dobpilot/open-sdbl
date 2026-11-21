## ADDED Requirements

### Requirement: Resolve metadata on targets without thread support
Metadata finalization SHALL use no more workers than available parallelism and SHALL run synchronously when worker threads are unavailable.

#### Scenario: Single-threaded WebAssembly
- **WHEN** at least 256 fields are resolved on a target without worker threads
- **THEN** metadata resolution completes synchronously with deterministic results
