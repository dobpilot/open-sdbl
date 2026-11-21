## ADDED Requirements

### Requirement: Preserve measured empty legacy processor attribute declarations
The SDK SHALL preserve an explicit empty Pattern declaration in the measured legacy data processor profile as a known empty declared-type collection. This metadata SHALL NOT be interpreted as runtime Any semantics. A complete attribute collection containing such declarations SHALL preserve every native identity and its order. Unsupported, malformed, duplicate or incomplete descriptions SHALL remain unknown without exposing partial membership.

#### Scenario: Native conversion processor declarations
- **WHEN** the retained BUH conversion processor is decoded
- **THEN** all 43 own attributes retain native XML identity and order, four explicit empty declarations remain known empty and the other declarations preserve their types and qualifiers

#### Scenario: Malformed declaration remains unknown
- **WHEN** any declaration, attribute identity or membership in this profile is malformed, unsupported, duplicate or incomplete
- **THEN** the SDK does not publish a complete attribute list or a partially committed prefix

#### Scenario: Native textual attribute comments
- **WHEN** an attribute in the retained profile carries a nonempty textual comment
- **THEN** its comment and complete membership are retained, while a non-string comment remains unsupported
