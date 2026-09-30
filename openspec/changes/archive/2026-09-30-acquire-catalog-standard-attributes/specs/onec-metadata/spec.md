## ADDED Requirements

### Requirement: Acquire catalog standard attribute settings from Config
The library SHALL expose the configured code type, code length and description length on a catalog owner descriptor when the acquired Config layout is recognized. Zero lengths SHALL be preserved as configuration settings and SHALL NOT imply that a corresponding BSL object property is inaccessible. Unknown layouts SHALL remain unknown and SHALL NOT be inferred from SQL columns. Nested custom attributes SHALL NOT inherit their owner settings.

#### Scenario: Numeric code and zero description length
- **WHEN** the acquired catalog descriptor declares numeric Code with length 9 and Description length 0
- **THEN** its standard settings contain Number, 9 and 0

#### Scenario: String code
- **WHEN** a recognized catalog descriptor declares string Code
- **THEN** its standard settings contain String and the acquired lengths

#### Scenario: Unknown layout
- **WHEN** the catalog settings are truncated or contain an unknown code type
- **THEN** standard settings are absent instead of guessed
