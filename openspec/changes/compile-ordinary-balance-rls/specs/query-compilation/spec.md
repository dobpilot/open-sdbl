## ADDED Requirements

### Requirement: Compile measured ordinary balance row authorization
An explicitly enabled Restricted compilation SHALL provide a single-statement execution contract for the measured ordinary PostgreSQL accumulation Balance SELECT forms. It SHALL distinguish forbidden selected balances from an empty permitted result without application SQL parsing. It SHALL preserve selected-principal decisions, separation and current parameter bindings. Unsupported ordinary forms SHALL fail explicitly.

#### Scenario: Permitted balance
- **WHEN** the measured ordinary Balance read selects a balance admitted by its dimension restriction
- **THEN** executing the compiled statement returns its permitted data with no forbidden marker

#### Scenario: Forbidden balance
- **WHEN** the same read selects a balance outside the restriction
- **THEN** the compiled statement returns a structured forbidden marker even if there are no permitted data rows
- **AND** the host must fail before exposing any result

#### Scenario: ALLOWED
- **WHEN** the statement carries ALLOWED
- **THEN** existing filtering remains and excluded balances produce no data

#### Scenario: Explicit manager dimension filter
- **WHEN** the Balance source has the measured equality filter selecting category a or b
- **THEN** the same filter constrains both data and forbidden detection

#### Scenario: Unsupported shape
- **WHEN** the enabled ordinary restricted read uses an unverified join, aggregate, batch, backend or predicate form
- **THEN** compilation fails explicitly without returning executable SQL

### Requirement: Inspect authorization before logical materialization
The compiler SHALL identify the appended forbidden-row service column structurally. Consumers SHALL inspect every row before logical materialization or publication, refuse any true marker, and refuse missing or malformed markers. Both authorization detection and permitted data SHALL execute in a single database statement.

#### Scenario: One snapshot
- **WHEN** a host executes the returned statement in its existing READ ONLY READ COMMITTED transaction
- **THEN** data and authorization detection observe the same statement snapshot

#### Scenario: Witness payload
- **WHEN** a returned row carries the forbidden marker
- **THEN** the host raises an access error without decoding its NULL payload as logical values
