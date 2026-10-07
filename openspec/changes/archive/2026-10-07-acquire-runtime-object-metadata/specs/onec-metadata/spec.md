## ADDED Requirements

### Requirement: Acquire business-process object metadata
The SDK SHALL expose optional measured number type, length and allowed-length settings, complete ordered own attribute and tabular-section GUID collections, and complete ordered canonical standard-attribute filling-check policies on business-process owner descriptors. Existing nested descriptors SHALL retain names, synonyms, qualified types, filling values and checking settings. Section attributes SHALL use existing ordered membership. Generated object and reference type identities SHALL remain distinct. Unknown, malformed, duplicate or incomplete collections SHALL be None; verified empty collections SHALL be Some(empty). Other owner kinds SHALL not receive these fields. Route-point behavior SHALL remain unchanged.

#### Scenario: Native business process has own fields and sections
- **WHEN** native configuration contains distinguishable ordered own attributes and section attributes
- **THEN** complete owner and section lists preserve configuration order and exclude each other's members

#### Scenario: Unknown or incomplete process metadata
- **WHEN** a required member is missing or a layout or setting code is unknown
- **THEN** the corresponding acquisition remains unknown without a shortened list or guessed default

#### Scenario: Acquired process survives snapshot construction
- **WHEN** recognized descriptors pass through snapshot construction or selective resource acquisition
- **THEN** acquired numbering, policies and complete member lists remain available

### Requirement: Acquire accumulation-register membership and kind
The SDK SHALL expose optional measured Balance or Turnovers kind and complete ordered own dimension and resource GUID collections on accumulation-register owner descriptors. Existing nested descriptors SHALL provide names, synonyms and qualified types. Kind SHALL NOT be inferred from physical storage. Unknown codes, malformed, duplicate, incomplete or unmeasured collections SHALL remain None; verified empty collections SHALL be Some(empty). Other object kinds SHALL receive None.

#### Scenario: Native register has ordered dimensions and resources
- **WHEN** a recognized native balance or turnover register declares several members
- **THEN** its measured kind and independently complete ordered dimensions and resources are retained without attributes or other owners' fields

#### Scenario: Malformed register collection
- **WHEN** a collection count disagrees or any member is missing or duplicated
- **THEN** the affected whole collection is unknown, never partially acquired

#### Scenario: Register survives acquisition
- **WHEN** recognized register descriptors pass through snapshot construction and selective acquisition
- **THEN** all already acquired kind and collection settings remain unchanged
