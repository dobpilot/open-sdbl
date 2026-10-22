# User interface language

## ADDED Requirements

### Requirement: Preserve the selected user interface language identity
The decoded UserData SHALL expose the selected interface-language GUID from the measured 38-field user record. A selected GUID, a known nil GUID and an unsupported or incomplete layout SHALL remain distinguishable. InfoBaseUser returned through asynchronous and blocking acquisition SHALL preserve this value without additional SQL. No language SHALL be inferred from names, configuration defaults or operating-system locale.

#### Scenario: Native selected and default language
- **WHEN** the measured Russian, English and default-user records are decoded
- **THEN** their selected GUIDs match the native metadata and the default record carries a known nil GUID

#### Scenario: Unknown language structure
- **WHEN** the language has a wrong type or GUID, or the record layout is unsupported or incomplete
- **THEN** the language is unknown and previously decodable identity, names, roles and separator data are not discarded merely because the language is unknown

#### Scenario: Existing acquisition and confidentiality
- **WHEN** an ordinary user row is acquired
- **THEN** its decoded language is retained without mutating input bytes or exposing password hashes through the new API, Debug, errors or logs
