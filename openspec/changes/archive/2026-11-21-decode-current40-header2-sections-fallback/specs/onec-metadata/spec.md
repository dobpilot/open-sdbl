## ADDED Requirements

### Requirement: Fall back to measured current40 version2 document sections
When the existing section lookup is unknown and own attributes are complete, the decoder SHALL recognize the measured current40 version2 owner/section/field profile and preserve complete ordered section membership and qualified field declarations. It SHALL validate the complete newly supported set before publication.

#### Scenario: Whole native equipment-installation document
- **WHEN** the retained native document has its ten own attributes and two version2 sections with ten fields
- **THEN** all memberships, UUIDs, names, order and twenty qualified type rows match native XML

#### Scenario: Malformed fallback or missing complete owns
- **WHEN** owner/late-part/late-field shape, synonym, String comment, flags, tail, count, declaration, descriptor uniqueness or identity collisions are invalid, or complete own metadata is unavailable
- **THEN** no NEW section/type prefix is published through this fallback
- **AND** previously known own metadata and old settings retain their behavior

#### Scenario: Previously known sections
- **WHEN** the prior lookup already returns known sections, including owner version2 with section version3
- **THEN** the new fallback is not used and the existing projection remains unchanged

#### Scenario: Existing unrelated profiles
- **WHEN** existing legacy37/38, modern3 or catalog metadata is acquired
- **THEN** its previous behavior remains unchanged, including existing partial modern3 behavior
