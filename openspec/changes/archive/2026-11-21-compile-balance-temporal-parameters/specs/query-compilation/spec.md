## ADDED Requirements

### Requirement: Typed balance temporal parameters
The compiler SHALL accept a date or point-in-time boundary as the period argument of an accumulation Balance table. A point SHALL preserve its date, optional metadata object identity and physical RRRef separately. Boundary inclusion SHALL be explicit; constructing a boundary without a kind SHALL select Including. Raw dates and raw points SHALL remain exclusive cuts. Other positions SHALL explicitly diagnose unsupported temporal values rather than dropping a reference or inclusion.

#### Scenario: Multiple recorders at one second
- **WHEN** three recorders of two acquired document kinds share a date with movements 1, 10 and 100 in measured order
- **THEN** their exclusive point balances SHALL be 0, 1 and 11, and inclusive balances SHALL be 1, 11 and 111

#### Scenario: Empty references in a balance period
- **WHEN** a boundary point has no reference or a typed empty reference
- **THEN** Including SHALL include all movements at its second and Excluding SHALL exclude all of them
- **AND** the public model SHALL preserve the distinction between absent and typed-empty references

#### Scenario: Metadata and policy are not bypassed
- **WHEN** a temporal Balance parameter is compiled
- **THEN** acquired storage type numbers, existing date storage offsets, access decisions, restrictions and separation SHALL remain in force
- **AND** missing or unsupported reference metadata SHALL cause an explicit diagnostic

#### Scenario: Unsupported scalar position
- **WHEN** a temporal boundary is supplied as an ordinary scalar projection or an unimplemented virtual-table argument
- **THEN** compilation SHALL return an explicit diagnostic

### Requirement: Point-in-time logical result recipe
A projected standard PointInTime field SHALL identify its date and typed-reference cells through the existing logical result projection API, retaining the existing physical labels and column types. Consumers SHALL NOT need to infer the grouping from label spelling.

#### Scenario: Document point result
- **WHEN** a document PointInTime field is selected under an alias
- **THEN** one logical result position SHALL identify both physical members and the reference metadata type
