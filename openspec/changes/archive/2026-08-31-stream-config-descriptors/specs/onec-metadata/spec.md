## ADDED Requirements

### Requirement: Project Config descriptors while parsing
The metadata decoder SHALL validate each bare-GUID Config resource and project
owner and nested descriptors without retaining a complete generic value tree.
It SHALL preserve descriptor source order, marker, object GUID, name, synonyms,
optional comment, and inherited field purpose. Intermediate retained state
SHALL be bounded by decoded input, output descriptors, nesting depth, a
four-candidate window per active list, and scalar-only lists required by a
matching descriptor.

#### Scenario: Owner and nested descriptors
- **WHEN** one Config resource contains an owner descriptor and nested attribute
  descriptors among unrelated complex branches
- **THEN** streaming projection returns the same descriptors in source order
  without retaining unrelated branches

#### Scenario: Inherited collection purpose
- **WHEN** a recognized register collection contains a nested field descriptor
- **THEN** the field receives the same inherited dimension, resource, or
  attribute purpose as the generic recursive projection

#### Scenario: Descriptor presentation fields
- **WHEN** a matching descriptor has localized synonym pairs and an immediately
  following nonempty comment
- **THEN** both fields are preserved exactly

#### Scenario: Malformed Config branch
- **WHEN** any branch of a bare-GUID Config resource is malformed
- **THEN** parsing fails positionally even when that branch cannot match a
  descriptor
