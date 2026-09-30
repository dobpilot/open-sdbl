## ADDED Requirements

### Requirement: Acquire configured catalog hierarchy flag
The library SHALL expose the configured Hierarchical flag as an optional boolean on a recognized acquired catalog owner descriptor. Unknown values or layouts SHALL remain unknown. Nested descriptors SHALL NOT inherit the owner flag. The flag SHALL NOT be inferred from SQL, other catalog settings or BSL source and SHALL NOT implement object setter or saving policies.

#### Scenario: Hierarchical catalog
- **WHEN** a recognized catalog owner declares Hierarchical=true
- **THEN** its descriptor exposes true

#### Scenario: Nonhierarchical catalog
- **WHEN** a recognized catalog owner declares Hierarchical=false
- **THEN** its descriptor exposes false

#### Scenario: Unknown flag
- **WHEN** the hierarchy setting is missing or unrecognized
- **THEN** the descriptor exposes no flag instead of guessing false
