## ADDED Requirements

### Requirement: Catalog hierarchy type
The SDK SHALL acquire the measured typed catalog hierarchy mode independently from its existing hierarchical flag. The measured folders-and-items and items-only modes SHALL be distinguishable. Unknown forms or codes SHALL remain None. Existing complete catalog owner GUIDs SHALL retain their order and distinguish unknown from verified empty membership.

#### Scenario: Both native hierarchy modes
- **WHEN** recognized native catalogs use folders-and-items and items-only hierarchies
- **THEN** the corresponding distinct typed modes survive acquisition through MetadataSnapshot

#### Scenario: Nonhierarchical catalog
- **WHEN** a recognized catalog disables its hierarchical flag
- **THEN** its configured hierarchy mode remains independently acquired

#### Scenario: Catalog owners
- **WHEN** a recognized catalog has zero, one or multiple owners
- **THEN** the complete ordered owner GUID list survives acquisition
