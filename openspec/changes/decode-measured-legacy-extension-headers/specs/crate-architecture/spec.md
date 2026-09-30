## ADDED Requirements

### Requirement: Decode measured legacy extension object headers
Extension object acquisition SHALL recognize legacy header UUID and name shapes confirmed by native XML export. The measured Adopted belonging SHALL be retained. A modified-property identifier SHALL NOT be reported as an adoption target. Existing whole-or-unknown object-list completeness SHALL remain unchanged.

#### Scenario: Native legacy adopted object
- **WHEN** a descriptor uses the measured legacy header and Adopted tail
- **THEN** its own UUID, name and Adopted belonging are decoded without an inferred target link

#### Scenario: Unsupported legacy tail
- **WHEN** a recognized legacy identity and name have an unsupported tail
- **THEN** the identity and name remain available but belonging and target link remain unknown

#### Scenario: Missing own header
- **WHEN** any catalog object still lacks its own decoded header
- **THEN** the acquired object list remains unknown rather than partial
