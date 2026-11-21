## ADDED Requirements

### Requirement: Preserve direct restriction parameter origin
Bound query compilation SHALL identify an absent direct parameter token inside an access restriction as a missing session parameter, preserving its exact name through the existing diagnostic. Ordinary query parameters SHALL retain their existing classification. Query parameter values SHALL NOT satisfy restriction parameters.

#### Scenario: Direct restriction parameter absent
- **WHEN** a bound access predicate reads an absent direct session parameter
- **THEN** compilation returns the existing structured missing-session-parameter diagnostic with its name

#### Scenario: Session value supplied
- **WHEN** the host retries compilation with that session parameter value
- **THEN** the predicate compiles using that value

#### Scenario: Ordinary query parameter absent
- **WHEN** an ordinary query reads an absent parameter
- **THEN** the diagnostic remains an ordinary missing query parameter

#### Scenario: Query value has the same name
- **WHEN** only a query parameter has the restriction parameter name
- **THEN** it does not satisfy the restriction and the missing session parameter is still reported
