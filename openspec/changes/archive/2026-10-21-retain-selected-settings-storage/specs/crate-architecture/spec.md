## ADDED Requirements
### Requirement: Preserve supported settings storage in selected resolution
Selected and cached metadata resolution SHALL retain acquired physical facts for the already supported common and system settings stores, including their required separator field mappings, even without a Config object owner. It SHALL use the same fixed storage identities as existing settings load compilers and SHALL NOT include unrelated ownerless tables.
#### Scenario: Settings load parity
- **WHEN** acquired supported settings tables and required separator descriptors are supplied
- **THEN** selected, cached and whole metadata compile equivalent settings load SQL and enforce equivalent complete scope validation
#### Scenario: Missing separator descriptor
- **WHEN** supported settings storage contains a known separator whose required descriptor is not supplied
- **THEN** strict selected and cached resolution report the missing descriptor GUID without issuing unfiltered settings SQL
#### Scenario: Unrelated table
- **WHEN** an acquired ownerless table is not a supported settings store
- **THEN** selected resolution excludes it unless existing reached-object rules select it
#### Scenario: Snapshot validation
- **WHEN** acquired settings storage changes the selected snapshot facts
- **THEN** the existing snapshot fingerprint and prepared-query validation rules remain enforced
