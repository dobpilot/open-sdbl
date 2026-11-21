## ADDED Requirements

### Requirement: Complete measured BUH document attributes
The SDK SHALL retain complete ordered own-attribute membership, identities, names and qualified declarations for measured class40/53 documents with version2/8 identities. Unknown, malformed, missing, duplicate or unsupported declarations SHALL keep the collection unknown, never partial. Existing modern forms SHALL remain supported.

#### Scenario: Native credit document
- **WHEN** the retained native credit-document resource is decoded
- **THEN** all 23 own attributes agree with paired native XML

#### Scenario: Invalid member
- **WHEN** a member identity, wrapper or type is invalid, missing or duplicated
- **THEN** no partial membership is returned

#### Scenario: Modern forms
- **WHEN** modern identities use measured 5/5 or 6/7 wrappers
- **THEN** complete ordered membership and types remain available
