## ADDED Requirements

### Requirement: Acquire measured legacy report-event catalog sections atomically
The SDK SHALL preserve complete ordered section and field membership and qualified field declarations for the measured legacy report-event catalog profile. It SHALL validate the entire section set before publishing any newly supported membership or declaration. Invalid or incomplete sections SHALL remain unknown without a partial prefix, while preserving existing own-attribute metadata.

#### Scenario: Complete native catalog sections
- **WHEN** the retained native report-event catalog contains its ПричиныОтказа section and four fields
- **THEN** section and field UUIDs, names, order, types and qualifiers match native XML
- **AND** the seven already acquired own attributes retain their declarations

#### Scenario: Invalid late section or field
- **WHEN** a late section or field has an unknown shape, invalid count/header/declaration, missing or duplicate descriptor, duplicate identity or collision with owner or own attributes
- **THEN** no section prefix or field declarations are published
- **AND** existing own attributes remain unchanged

#### Scenario: Existing document and modern catalog profiles
- **WHEN** previously supported document sections or modern catalogs are acquired
- **THEN** their existing projection remains unchanged
