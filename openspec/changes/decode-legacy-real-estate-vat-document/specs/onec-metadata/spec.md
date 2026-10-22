## ADDED Requirements

### Requirement: Acquire measured legacy real-estate VAT document metadata
The decoder SHALL preserve complete ordered own attributes and section/field membership with qualified declarations for the measured legacy real-estate VAT recovery document. It SHALL validate each newly supported complete collection before publication, preserving known own metadata when sections fail. Unsupported metadata SHALL remain unknown without partial prefixes.

#### Scenario: Whole native document
- **WHEN** the retained native CF/XML document is acquired
- **THEN** its10 own UUIDs,2 section UUIDs,24 ordered fields and48 qualified type rows match XML
- **AND** Characteristic TypeSet references retain their exact generated TypeIds

#### Scenario: Malformed own collection
- **WHEN** owner or late own shape, flags, String identity/comment, count, type or descriptor is invalid, duplicated or incomplete
- **THEN** no own or dependent section prefix is published

#### Scenario: Malformed section collection
- **WHEN** a late part or field has an invalid shape, flags, header/comment, count, type, descriptor or duplicate/colliding identity
- **THEN** no section/field prefix is published
- **AND** already complete own metadata remains unchanged

#### Scenario: Existing profiles
- **WHEN** previously supported document38/40 or catalog profiles are acquired
- **THEN** their previous metadata projection remains unchanged
