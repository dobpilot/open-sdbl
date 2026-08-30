## ADDED Requirements

### Requirement: Provide indexed metadata identity lookup
The resolved metadata snapshot SHALL expose GUID-backed object and attribute
IDs, stable numeric standard-field IDs, and indexed lookup by object GUID,
normalized kind/name, owner/name, and database reference type number. Expected
lookup time SHALL be O(1), excluding normalization and result cloning.
Ambiguity and the absence of a Config GUID for a standard field SHALL be typed
outcomes.

#### Scenario: Object GUID by kind and name
- **WHEN** an application looks up a unique object by supported kind and
  logical name
- **THEN** the snapshot returns its real 16-byte metadata GUID

#### Scenario: Attribute GUID by owner and name
- **WHEN** an application looks up a custom attribute by owner GUID and logical
  name
- **THEN** the snapshot returns the attribute's real 16-byte metadata GUID

#### Scenario: Standard field lookup
- **WHEN** an application looks up `Код`, `Наименование`, or another standard
  field
- **THEN** field lookup returns its numeric standard-field ID and strict
  attribute-GUID lookup reports that the field has no metadata GUID

#### Scenario: Reference type lookup
- **WHEN** a physical RTRef value identifies a DBNames table number
- **THEN** indexed lookup returns the corresponding object GUID without
  guessing from a physical table name
