# Legacy user interface language

## ADDED Requirements

### Requirement: Decode measured complete legacy language records
The selected interface-language identity SHALL also be decoded from complete measured 31- and 37-field user records without separation bindings. Selected non-nil, known nil and unknown SHALL remain distinct. Existing identity, names and roles SHALL be preserved. Acquisition SHALL use the existing shared row decoder without additional SQL.

#### Scenario: Native legacy language choices
- **WHEN** own native Russian, English and default records in either measured legacy layout are decoded
- **THEN** the selected language GUID or known nil matches the official platform and identity, names and roles remain unchanged

#### Scenario: Malformed or unmeasured records
- **WHEN** a record is truncated, has inconsistent optional counts, unknown suffix, wrong language type or GUID, uses form 32 or carries separation bindings
- **THEN** its selected language remains unknown

#### Scenario: Shared database acquisition
- **WHEN** a complete measured legacy record passes through the ordinary row decoder used by asynchronous and blocking acquisition
- **THEN** its selected language is preserved without additional SQL or disclosure of password data
