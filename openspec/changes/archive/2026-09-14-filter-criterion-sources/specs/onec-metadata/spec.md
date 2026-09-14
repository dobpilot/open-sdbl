## ADDED Requirements

### Requirement: Filter criteria are decoded from Config
A Config resource that declares a filter criterion SHALL be decoded into
the criterion's name and the GUIDs of the fields it searches, and the
snapshot SHALL look a criterion up by name.

#### Scenario: Criterion resource
- **WHEN** the Config resource of a filter criterion is parsed
- **THEN** its name and content field GUIDs are available on the snapshot
