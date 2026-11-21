## ADDED Requirements

### Requirement: Compile a scoped common-settings load
The SDK SHALL compile a read-only PostgreSQL common-settings load for exact user, object and settings keys and explicit complete string or numeric separator states. It SHALL derive physical separator/flag pairs from acquired live columns and DBNames GUID mappings. Missing, duplicate, extra, unsupported or ambiguous scope SHALL fail before execution. An explicitly inactive separator SHALL match false and its typed empty stored value; active separators SHALL match true and their exact typed values. All literals and identifiers SHALL use the existing dialect renderer. The read SHALL return at most two payload rows so the host can reject ambiguity. Other backends SHALL fail explicitly until supported.

#### Scenario: Complete active scope
- **WHEN** every observed separator has one explicit active string state
- **THEN** SQL constrains all key, value and use columns and returns at most two payloads

#### Scenario: Inactive scope
- **WHEN** a separator is explicitly inactive
- **THEN** SQL matches its false use flag and measured empty stored value

#### Scenario: Unknown scope
- **WHEN** a separator state or its physical GUID mapping is missing or ambiguous
- **THEN** compilation fails rather than omitting a predicate

#### Scenario: Literal and backend handling
- **WHEN** a key contains quotes
- **THEN** it is rendered through the existing PostgreSQL literal renderer
- **WHEN** the backend is unsupported
- **THEN** compilation fails explicitly

#### Scenario: Active empty separator value
- **WHEN** a resolved active state contains an empty string
- **THEN** SQL retains a true use flag and the empty value predicate
- **AND** it remains distinct from an inactive state with a false use flag

### Requirement: Canonical globally disabled settings scope
A resolved false global condition SHALL compile to use=true and the typed empty value, independently of any supplied use/value parameters. The scope API SHALL distinguish that state from an inactive separator, which uses false. Unsupported value types SHALL fail before execution.

#### Scenario: Numeric condition is false
- **WHEN** a numeric separator has a resolved false global condition
- **THEN** SQL uses a true flag and numeric zero

#### Scenario: String condition is false
- **WHEN** a string separator has a resolved false global condition
- **THEN** SQL uses a true flag and empty string

#### Scenario: Active numeric value
- **WHEN** the resolved active state contains a number
- **THEN** its SQL literal uses the existing exact decimal renderer
- **AND** a string value supplied for the numeric column is rejected
