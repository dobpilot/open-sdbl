## ADDED Requirements

### Requirement: Characteristic object settings
The SDK SHALL expose measured code and description lengths and qualified value types for recognized characteristic chart descriptors. Unknown or incomplete layouts and unknown type definitions SHALL remain unknown. Direct parsing SHALL preserve named type references; acquired snapshots SHALL expand them with the existing named type resolver. Existing catalog and document projections SHALL remain unchanged.

#### Scenario: Native qualified characteristic
- **WHEN** the measured class 34 native resource is parsed
- **THEN** lengths and value types match the native XML including qualifiers and order

#### Scenario: Unknown declaration
- **WHEN** a characteristic declaration contains an unsupported type
- **THEN** its settings remain unknown instead of returning a partial type list

### Requirement: Complete characteristic own attributes
The SDK SHALL expose a complete ordered collection of own attribute GUIDs for recognized characteristic charts, linked to descriptors with names and qualified declared types. A verified empty collection SHALL be an empty vector, whereas incomplete or unsupported collections SHALL remain unknown. Common attributes and tabular section fields SHALL NOT be included. Existing reference type identity SHALL represent the native reference TypeId.

#### Scenario: Native own attribute
- **WHEN** the measured chart contains its one own string attribute
- **THEN** the collection contains exactly its GUID and its descriptor retains the native name and string qualifiers

#### Scenario: Empty or corrupt collection
- **WHEN** a recognized collection is empty or incomplete
- **THEN** the former is a known empty vector and the latter is unknown
