## ADDED Requirements

### Requirement: Discover measured defined-type identity in bounded headers
Bounded Config header discovery SHALL expose the generated identity of a defined type recognized by the same measured wrapper rule as full descriptor decoding. Unknown wrapper forms SHALL remain distinct from measured absent GUIDs. Existing object, reference and route-point identities and bounded parsing errors SHALL remain unchanged.

#### Scenario: Native defined-type parity
- **WHEN** a native measured defined-type description is decoded as a header and as a full descriptor
- **THEN** both expose the same defined-type GUID without constructing detailed attribute descriptors in the header path

#### Scenario: Unknown or malformed shape
- **WHEN** the defined-type wrapper is unknown or its owner header does not match the resource
- **THEN** the header does not invent a known defined-type identity and retains existing parse error behavior

#### Scenario: Measured absence representation
- **WHEN** a recognized defined-type wrapper contains a nil generated type GUID
- **THEN** its identity is absent rather than unknown
