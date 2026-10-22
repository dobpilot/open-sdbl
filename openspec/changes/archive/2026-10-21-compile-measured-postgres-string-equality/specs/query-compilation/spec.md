## ADDED Requirements

### Requirement: Compile measured PostgreSQL scalar string equality
PostgreSQL scalar string equality SHALL preserve case and ignore trailing ASCII spaces. Comparing a string with a Number, Boolean or DateTime SHALL not convert the incompatible operand to a string or fail with a provider cast error. Field authorization and row restrictions SHALL remain enforced.

#### Scenario: Measured catalog string values
- **WHEN** AbC is compared with AbC, abc, or AbC followed by a space
- **THEN** the equality results are true, false, and true respectively

#### Scenario: Incompatible scalar equality
- **WHEN** a non-null string is compared with a Number, Boolean or DateTime
- **THEN** equality is false without an implicit cast

#### Scenario: SQL null
- **WHEN** either operand is SQL NULL
- **THEN** the comparison retains SQL NULL propagation

#### Scenario: Contextual CASE boundary
- **WHEN** a CASE operand requires existing contextual type selection
- **THEN** that existing compilation path remains unchanged

#### Scenario: Provider boundary
- **WHEN** the target is SQL Server or the operator is not equality
- **THEN** this bounded correction does not change that compilation path
