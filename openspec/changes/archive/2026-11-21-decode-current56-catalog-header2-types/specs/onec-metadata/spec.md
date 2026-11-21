## ADDED Requirements

### Requirement: Retain measured current56 catalog own type declarations
The decoder SHALL retain complete qualified own declarations for the measured class56/61 header2 catalog profile while preserving existing complete membership and unrelated metadata.

#### Scenario: Native extension-version catalog
- **GIVEN** the whole native CF/XML with two known own IDs, standard fields and empty sections
- **WHEN** the resource is decoded
- **THEN** both ordered own IDs retain exact String and DateTime declarations and other known metadata remains unchanged

#### Scenario: Malformed measured profile
- **GIVEN** a malformed late row, header, type, tail, count, descriptor, collision or unknown shape
- **WHEN** declaration projection is attempted
- **THEN** it publishes no new declaration prefix and preserves independently acquired metadata

#### Scenario: Permission isolation
- **GIVEN** declaration projection succeeds
- **WHEN** the result is used by a consumer
- **THEN** role grants, access-check policy and privilege remain unchanged
