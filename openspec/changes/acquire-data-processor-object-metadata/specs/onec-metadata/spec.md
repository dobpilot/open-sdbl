## ADDED Requirements

### Requirement: Acquire data processor object metadata
The SDK SHALL expose optional complete ordered own attribute and tabular-section GUID collections and generated object type identity for measured data processor descriptions. Nested descriptors SHALL retain qualified types and section membership. Unknown, malformed, duplicate or incomplete collections SHALL remain None; verified empty collections SHALL be Some(empty). Other kinds SHALL not receive these fields.

#### Scenario: Native ordered object members
- **WHEN** a native data processor declares distinguishable ordered fields and sections
- **THEN** acquired owner and section lists preserve order and separate ownership, and object type differs from manager type

#### Scenario: Empty and incomplete collections
- **WHEN** a collection is verified empty or any member is malformed, missing or duplicated
- **THEN** emptiness is known and incomplete membership is unknown without a partial list

#### Scenario: Selective acquisition retains object metadata
- **WHEN** selective acquisition discards source resource bytes
- **THEN** already acquired data processor fields, section membership and object type remain available
