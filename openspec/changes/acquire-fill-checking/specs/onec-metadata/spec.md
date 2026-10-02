## ADDED Requirements

### Requirement: Acquired filling-check settings
The SDK SHALL expose measured DontCheck and ShowError settings for own attributes, tabular-section fields and common attributes through acquired descriptors. Unsupported forms or codes SHALL remain unknown. It SHALL expose a complete ordered catalog standard-attribute settings list with the owning descriptor; sparse, incomplete or unsupported settings SHALL NOT be returned as a complete list. No settings SHALL be inferred from values, lengths, object names or BSL source.

#### Scenario: Native attribute settings
- **WHEN** a recognized native field declares DontCheck or ShowError
- **THEN** its acquired setting matches the native XML and retains its field identity

#### Scenario: Complete standard settings
- **WHEN** a recognized catalog stores the measured complete standard property table
- **THEN** the acquired ordered list retains every standard name and its mode, on that catalog only

#### Scenario: Incomplete or unknown standard settings
- **WHEN** a standard property count, entry, mode or name is unsupported
- **THEN** the entire standard settings list is unknown without partial success or guessed defaults
