## ADDED Requirements

### Requirement: Preserve measured legacy report-deliveries own attributes
The decoder SHALL preserve the complete ordered own-attribute membership and qualified declarations of the measured legacy BUH report-deliveries catalog when the whole owner and collection are recognized. Invalid collections SHALL NOT publish partial membership or declaration prefixes.

#### Scenario: Complete native catalog
- **WHEN** the retained native CF/XML catalog contains its 16 measured own attributes
- **THEN** their UUIDs, names, order, types, references and qualifiers match XML, including the date-only start-of-period attribute

#### Scenario: Invalid legacy collection
- **WHEN** owner (including malformed synonym structure or non-string text), count, last-row wrapper/header/type, descriptor uniqueness or identity uniqueness is invalid, or an attribute aliases its owner
- **THEN** own membership stays unknown and no declaration prefix is published

#### Scenario: Modern catalog
- **WHEN** an existing supported modern catalog is decoded
- **THEN** its metadata projection remains unchanged
