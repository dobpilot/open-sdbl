## ADDED Requirements

### Requirement: Acquire measured legacy data processor attributes
The SDK SHALL retain complete ordered own attribute identities and qualified declared types from the measured legacy data processor profile. Unsupported or malformed identities, duplicate or incomplete membership and unreadable declarations SHALL remain unknown. Generated type-family references SHALL retain their original declared identity and SHALL NOT be relabelled as a concrete metadata object. Other owner kinds and tabular-section fields SHALL retain their existing profile boundaries.

#### Scenario: Native legacy attributes
- **WHEN** the independently validated legacy data processor declares two qualified string fields and one ExchangePlanRef type-set field
- **THEN** the owner exposes all three real attribute GUIDs in native XML order and the nested descriptors retain the original declarations

#### Scenario: Invalid or unsupported legacy attribute profile
- **WHEN** its identity, wrapper, declaration, owner or membership is malformed or unsupported
- **THEN** no complete legacy attribute collection is reported

#### Scenario: Other metadata profiles
- **WHEN** a Report, section field or modern data processor is acquired
- **THEN** its existing decoding boundaries and membership remain unchanged
