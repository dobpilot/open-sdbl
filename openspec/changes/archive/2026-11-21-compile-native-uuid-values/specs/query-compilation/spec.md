## ADDED Requirements

### Requirement: Compile typed UUID parameters and scalar attributes
On PostgreSQL the compiler SHALL accept a typed UUID parameter and preserve its UUID result kind. An acquired scalar attribute SHALL be classified as UUID only when its single unambiguous declaration is the measured UUID type and its physical representation is one binary column. Missing, ambiguous and compound declarations SHALL NOT cause arbitrary binary values to be reclassified. Raw binary parameters SHALL retain their existing behavior.

#### Scenario: Native equality and result type
- **WHEN** a UUID parameter equals the UUID stored in a declared scalar UUID attribute
- **THEN** the query matches that row and exposes a UUID result, while a different UUID does not match

#### Scenario: Derived UUID values
- **WHEN** a UUID parameter or attribute is projected through a subquery or UNION ALL
- **THEN** its canonical UUID identity and output kind are preserved without applying the storage conversion twice

#### Scenario: Binary and unknown declaration
- **WHEN** a binary field lacks an unambiguous scalar UUID declaration
- **THEN** the field and raw byte parameters retain their binary kind

#### Scenario: Unmeasured provider
- **WHEN** SQL Server compilation reads a newly recognized UUID attribute or binds a typed UUID parameter
- **THEN** it reports an explicit unsupported-feature diagnostic before execution without changing existing UUID(reference) support
