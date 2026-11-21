## ADDED Requirements

### Requirement: Connection-bound explicit read-only transactions
Async and blocking database sessions SHALL expose begin, commit, rollback and server-verified active-state operations on their already owned connection. Explicit transactions SHALL be non-nested and use READ COMMITTED, retaining existing provider read-only safeguards. Queries SHALL reuse the active transaction without implicit completion; outside it legacy automatic transaction behavior SHALL remain unchanged. Metadata and configuration acquisition SHALL reject an active explicit transaction. Errors SHALL roll back or disconnect when cleanup cannot be verified. Cancellation, close, Drop and connection loss SHALL never permit a claim of an active usable transaction. Blocking operations SHALL preserve closed-session and asynchronous-context diagnostics. SQL Server SHALL retain its existing requirement for a read-only login rather than promise server-enforced read-only mode absent in that provider.

#### Scenario: Multiple reads in one transaction
- **WHEN** a session begins an explicit transaction and runs multiple reads
- **THEN** all reads use the same connection and actual transaction until explicit commit or rollback

#### Scenario: Invalid lifecycle and cleanup
- **WHEN** begin is nested, finish is inactive, a query fails or a connection becomes unusable
- **THEN** lifecycle misuse returns an error and failed work rolls back or disconnects without reporting active

#### Scenario: Blocking parity
- **WHEN** blocking callers invoke lifecycle operations
- **THEN** the owned async session executes them under its limits and runtime rules
