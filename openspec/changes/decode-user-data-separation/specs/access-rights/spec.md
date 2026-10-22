## ADDED Requirements

### Requirement: Decode measured user data-separation bindings
The SDK SHALL expose an optional complete ordered list of string data-separation bindings for the measured native user-record layout. Known empty and unknown SHALL remain distinct. Unsupported layouts, unsupported binding values, duplicate names or inconsistent binding counts SHALL yield unknown bindings without discarding otherwise valid user identity and roles. Password fields SHALL NOT be exposed.

#### Scenario: Known empty bindings
- **WHEN** a measured-layout user record has zero bindings
- **THEN** bindings are known and empty

#### Scenario: Known string binding
- **WHEN** a measured-layout record contains a named string separator value
- **THEN** the complete binding name and value are retained

#### Scenario: Unsupported or incomplete bindings
- **WHEN** the record layout, a binding value or its count cannot be validated
- **THEN** bindings are unknown rather than empty or partial

#### Scenario: Historical native-readable layouts
- **WHEN** a record has the verified base length 31, 32 or 37 with consistent optional-field count and suffix kinds
- **THEN** the SDK retains the complete bindings using the same binding position as the measured current layout
- **AND** an inconsistent optional-field count or suffix remains unknown
