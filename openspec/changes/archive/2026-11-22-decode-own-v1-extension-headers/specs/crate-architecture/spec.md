## MODIFIED Requirements

### Requirement: Decode measured legacy extension object headers
Extension object acquisition SHALL recognize version 1 header UUID and name shapes confirmed by native XML export. For a tail with a valid pair count and complete property identifier and tag pairs, belonging SHALL be decoded as Own for `0` and Adopted for `1`. A modified-property identifier SHALL NOT be reported as an adoption target. Existing whole-or-unknown object-list completeness SHALL remain unchanged.

#### Scenario: Native legacy own object
- **WHEN** a descriptor uses the measured version 1 header with an Own tail and either zero property pairs or one complete property pair
- **THEN** its UUID, name and Own belonging are decoded and its target link is absent

#### Scenario: Native legacy adopted object
- **WHEN** a descriptor uses the measured legacy header and Adopted tail
- **THEN** its own UUID, name and Adopted belonging are decoded without an inferred target link

#### Scenario: Unsupported legacy tail
- **WHEN** a recognized legacy identity and name have an unknown belonging code, a mismatched count, or an incomplete or invalid property pair
- **THEN** the identity and name remain available but belonging and target link remain unknown

#### Scenario: Missing own header
- **WHEN** any catalog object still lacks its own decoded header
- **THEN** the acquired object list remains unknown rather than partial
