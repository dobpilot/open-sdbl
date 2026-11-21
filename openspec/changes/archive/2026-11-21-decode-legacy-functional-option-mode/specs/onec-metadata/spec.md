## ADDED Requirements

### Requirement: Preserve measured legacy functional-option settings
The SDK SHALL preserve the measured location identity and privileged retrieval mode of functional options in the recognized legacy identity profile. It SHALL validate the complete native structure and unique owner before publishing either field, SHALL preserve existing supported forms, and SHALL leave unknown forms unknown.

#### Scenario: Whole native legacy option
- **GIVEN** the unchanged BUH CF/XML option pair
- **WHEN** public metadata parsing decodes it
- **THEN** the owner identity, location GUID and privileged mode match XML
- **AND** global location resolution retains its existing responsibility

#### Scenario: Existing native Boolean controls
- **GIVEN** the retained independently measured false and true modern options
- **WHEN** they are decoded
- **THEN** both exact modes and their complete metadata remain unchanged

#### Scenario: Invalid old profile
- **GIVEN** a malformed header, owner mismatch, duplicate owner descriptor, invalid Content count/reference or unknown mode
- **WHEN** projection is attempted
- **THEN** no new partial or guessed settings are published
