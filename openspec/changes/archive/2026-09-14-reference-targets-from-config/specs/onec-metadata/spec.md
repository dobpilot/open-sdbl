## ADDED Requirements

### Requirement: Reference targets from the Config type description
The Config parser SHALL read, for the object a resource describes, the
reference type other objects name it by, and, for every attribute, the
reference types its type description names. A field whose SchemaStorage
target is unnamed SHALL take its reference targets from that description,
so a dereference reaches the tables the field may hold and no others. When
the description names anything that is not a stored object, the list SHALL
stay empty rather than narrow the field to a part of its targets.

#### Scenario: A composite attribute names its targets
- **WHEN** a Config resource declares an attribute with two reference
  types
- **THEN** both reference types are read into the attribute's descriptor

#### Scenario: Dereferencing a composite field
- **WHEN** a query dereferences a field whose SchemaStorage target is
  unnamed
- **THEN** only the tables its type description names are joined

#### Scenario: A type description beyond the stored objects
- **WHEN** the description names a type that no stored object carries
- **THEN** the field declares no targets and keeps its previous resolution
