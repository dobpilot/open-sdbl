## ADDED Requirements

### Requirement: Acquire complete exchange plan own attributes
The SDK SHALL expose optional complete ordered own ExchangePlan attribute GUID membership from measured native Config shapes. Member descriptors SHALL retain existing declared types and qualifiers. Unknown owner or row shapes, malformed counts, duplicate members, and missing typed member descriptors SHALL leave the collection None without a shortened list. A verified empty collection SHALL be Some(empty). Membership SHALL NOT be inferred from SQL, source names, or unrelated descriptors.

#### Scenario: Acquire native own attributes
- **WHEN** the paired native ExchangePlan Config resource contains eight own attributes
- **THEN** their ordered GUIDs and typed descriptors match Designer XML and EDT

#### Scenario: Preserve uncertainty
- **WHEN** an owner, collection or typed member is unsupported, duplicated, malformed or incomplete
- **THEN** own attribute membership remains None

#### Scenario: Verify absence
- **WHEN** a supported native owner has a correctly counted empty own attribute collection
- **THEN** own attribute membership is Some(empty)
