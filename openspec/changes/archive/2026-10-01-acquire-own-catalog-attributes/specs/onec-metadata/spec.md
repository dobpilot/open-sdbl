## ADDED Requirements

### Requirement: Acquire complete own catalog attribute membership
The library SHALL expose an optional complete list of own catalog attribute GUIDs on the acquired catalog owner descriptor, in configuration order. A recognized empty collection SHALL produce an empty list. Unknown or incomplete membership SHALL remain unknown rather than returning a partial list. Tabular-section attributes, common attributes and nested descriptors SHALL NOT be reported as own membership. Membership SHALL NOT be inferred from SQL or source code.

#### Scenario: Own and tabular attributes
- **WHEN** a catalog declares eight own attributes and three tabular-section attributes
- **THEN** its owner list contains the eight own GUIDs in native order and excludes the three tabular GUIDs

#### Scenario: Known empty membership
- **WHEN** a recognized catalog declares an empty own collection
- **THEN** its owner list is present and empty

#### Scenario: Incomplete membership
- **WHEN** the own collection count or any entry cannot be validated
- **THEN** the owner membership is absent rather than partial
