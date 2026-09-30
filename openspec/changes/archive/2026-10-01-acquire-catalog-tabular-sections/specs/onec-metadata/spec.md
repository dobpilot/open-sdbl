## ADDED Requirements

### Requirement: Complete catalog tabular section identities
The SDK SHALL expose an optional ordered list of own tabular section GUIDs on the owning catalog descriptor from recognized native Config structure. Nested fields SHALL NOT be included. Unknown, incomplete or duplicate collections SHALL yield None.

#### Scenario: Complete native collection
- **WHEN** a recognized catalog contains a complete tabular section collection
- **THEN** the owner exposes all section GUIDs in native order and nested descriptors expose None

#### Scenario: Known empty collection
- **WHEN** the recognized collection declares zero entries
- **THEN** the owner exposes Some of an empty list

#### Scenario: Unverifiable membership
- **WHEN** an entry header, count or uniqueness cannot be verified
- **THEN** the owner exposes None without guessing membership
