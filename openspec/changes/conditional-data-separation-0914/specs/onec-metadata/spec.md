## ADDED Requirements

### Requirement: Acquire the global conditional separation constant
The SDK SHALL retain the GUID reference to the common attribute global conditional separation constant independently of value/use parameter references and per-content conditions.

#### Scenario: Bound condition
- **WHEN** the measured Config descriptor binds a non-nil condition constant
- **THEN** acquired and resolved separator settings expose that GUID

#### Scenario: No condition
- **WHEN** the measured reference is nil
- **THEN** the settings expose no global condition
