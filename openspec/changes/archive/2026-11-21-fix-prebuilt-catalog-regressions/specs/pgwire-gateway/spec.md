## ADDED Requirements

### Requirement: Preserve catalog inventory across separator configurations
The prebuilt catalog SHALL describe supported relations and columns regardless of separator session values. Synthetic values used for schema discovery SHALL NOT affect actual statement execution or access decisions.

#### Scenario: Independent separator
- **WHEN** a relation requires a separator value supplied in a session
- **THEN** the prebuilt catalog exposes its permitted columns before any client session exists

### Requirement: Keep the gateway responsive during catalog construction
Asynchronous state acquisition SHALL perform catalog construction outside the async runtime worker driving gateway acceptance.

#### Scenario: Metadata refresh
- **WHEN** catalog construction for a replacement state takes time
- **THEN** the existing gateway can continue accepting connections until the replacement state is published

### Requirement: Use the configured database name for embedded states
Connections SHALL observe the configured backing database name in virtual catalog results even when their state was constructed from caller-supplied parts.

#### Scenario: State built from parts
- **WHEN** an embedded gateway serves a state constructed from parts with a configured database name
- **THEN** current_database and database names in catalog tables use the configured name
