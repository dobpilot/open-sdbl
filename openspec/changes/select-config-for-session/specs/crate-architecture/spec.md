## ADDED Requirements

### Requirement: Acquire selected configuration resources
The database crate SHALL let a consumer select Config resources to retain after it has inspected the `root` resource and the configuration descriptor named there. Both bootstrap resources SHALL be retained. Every other Config resource SHALL still be decoded when required for metadata resolution, while a resource not selected SHALL be discarded after decoding. The selected read SHALL preserve resolved metadata, extension information, rollback on failure, and the per-store retention limits of the whole read. The existing whole-configuration and metadata-only reads SHALL keep their contracts.

#### Scenario: Consumer selects code and object resources
- **WHEN** the consumer selects names from the configuration descriptor and excludes form assets
- **THEN** the answer includes both bootstrap resources and every selected resource, resolves the same metadata, and retains no excluded form asset

#### Scenario: Selection fails
- **WHEN** a bootstrap resource is absent or the selector rejects it
- **THEN** acquisition fails and rolls back the read-only transaction

### Requirement: Read access through a blocking session
A blocking session SHALL offer user and role-right reads through the existing asynchronous access functions on its owned database session and runtime. It SHALL create no extra connection or independent SQL path.

#### Scenario: Closed or asynchronous context
- **WHEN** an access read is called on a closed blocking session, including inside an asynchronous runtime
- **THEN** it answers `SessionClosed`
- **WHEN** the same read is called on an open blocking session inside an asynchronous runtime
- **THEN** it answers `AsyncContext` and leaves the session open
