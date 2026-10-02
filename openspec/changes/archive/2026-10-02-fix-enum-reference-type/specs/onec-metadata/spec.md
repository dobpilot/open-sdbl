## ADDED Requirements

### Requirement: Enumeration reference identities
The SDK SHALL acquire the measured EnumRef TypeId as the enumeration object reference type. It SHALL NOT substitute the enumeration manager TypeId. Catalog and document reference identity projection SHALL retain its existing layout.

#### Scenario: Enumeration filling value
- **WHEN** a document filling value names an acquired enumeration reference type
- **THEN** the enum object reference identity matches that type and differs from the manager type

#### Scenario: Native enumeration corpus
- **WHEN** recognized enumeration resources are compared with native Designer XML
- **THEN** every acquired reference TypeId matches its native EnumRef TypeId
