## ADDED Requirements

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
