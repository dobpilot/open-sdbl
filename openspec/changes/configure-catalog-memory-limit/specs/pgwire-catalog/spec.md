## ADDED Requirements

### Requirement: Configure catalog allocation budget
The gateway SHALL allow the operator to set a positive catalog query allocation budget in MiB using TOML or CLI. It SHALL default to 64 MiB, apply the configured budget independently to every catalog query in both simple and extended protocol, and reject invalid or overflowing values before connecting. Other resource limits SHALL remain unchanged.

#### Scenario: Increase the allocation budget
- **WHEN** a catalog query exceeds 64 MiB but fits within a configured larger budget
- **THEN** it succeeds without an allocation budget error

#### Scenario: Default budget remains bounded
- **WHEN** no budget is configured and a query exceeds 64 MiB
- **THEN** it fails with SQLSTATE `54001`

#### Scenario: Invalid budget
- **WHEN** the configured value is zero, negative, non-integral or overflows the byte representation
- **THEN** startup fails before the database is contacted
