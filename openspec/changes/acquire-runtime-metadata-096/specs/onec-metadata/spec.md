## ADDED Requirements

### Requirement: Acquire document runtime metadata
The SDK SHALL acquire measured document numbering, complete own attribute and tabular-section identities, and typed filling settings from recognized Config resources. Unknown settings and incomplete membership SHALL remain unknown. All declared reference types SHALL be preserved, including named sets; unresolved named declarations SHALL NOT be reported as complete.

#### Scenario: Native document has mixed attributes
- **WHEN** a recognized document declares multiple reference types, ValueStorage and filling values
- **THEN** its acquired descriptors preserve the full declaration and distinguish unknown filling values from known undefined values

#### Scenario: Incomplete collection
- **WHEN** any membership count or entry cannot be verified
- **THEN** the collection is absent rather than partial

### Requirement: Acquire catalog selection metadata
The SDK SHALL expose complete ordered catalog owner GUIDs and measured attribute indexing with distinct disabled, enabled and enabled-with-additional-ordering states. Unknown layouts SHALL remain unknown.

#### Scenario: Subordinate catalog
- **WHEN** a recognized catalog names verified owners
- **THEN** their metadata GUIDs are exposed in configuration order

#### Scenario: Unknown owner or indexing shape
- **WHEN** the acquired shape cannot be verified
- **THEN** no default or partial result is invented

### Requirement: Resolve acquired named declarations without narrowing
Metadata snapshot resolution SHALL expand known acquired named type declarations into every member and preserve qualifiers. Duplicate definitions, cyclic aliases or unknown member declarations SHALL remain unknown rather than exposing a partial list. Direct resource parsing SHALL retain the original named GUID; snapshot expansion SHALL happen once after all acquired descriptors are assembled.

#### Scenario: Event basis named type
- **WHEN** the document references the acquired basis definition containing 54 document and two catalog references
- **THEN** the snapshot descriptor contains all 56 reference members

#### Scenario: Cyclic named types
- **WHEN** named declarations form a cycle
- **THEN** the affected complete declaration is absent rather than recursively overflowing or dropping members
