## ADDED Requirements

### Requirement: Acquire report object metadata
The SDK SHALL expose optional complete ordered own report attribute and tabular section GUID collections and the acquired generated report Object type GUID. Unknown, malformed, duplicate or incomplete collections SHALL remain None; verified absence SHALL be Some(empty). Nested descriptors SHALL retain types, qualifiers and complete section membership.

#### Scenario: Acquire native report composition
- **WHEN** native report resources declare own attributes and a tabular section
- **THEN** ordered collections and nested column types agree with native Designer XML and the generated Object identity is distinct from Manager

#### Scenario: Preserve uncertainty
- **WHEN** a report header or member acquisition is unsupported or incomplete
- **THEN** the affected collection remains None without a shortened complete list

#### Scenario: Verify absence
- **WHEN** measured native report collections are empty
- **THEN** the SDK returns Some(empty)
