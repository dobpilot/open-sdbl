## ADDED Requirements

### Requirement: Acquire measured legacy news characteristic metadata
The SDK SHALL preserve the complete ordered own attribute collection and qualified declarations of the retained legacy news characteristic profile, together with its measured characteristic settings and existing reference identity. Unsupported, malformed, duplicate and incomplete descriptions SHALL remain unknown without publishing partial membership. Modern characteristic profiles SHALL retain their existing behavior. No runtime object semantics are inferred from this metadata profile.

#### Scenario: Native news characteristic metadata
- **WHEN** the retained native BUH news characteristic resource is decoded
- **THEN** all five own attributes, their types and qualifiers, code length 20, description length 50 and characteristic value declarations match the paired XML

#### Scenario: Malformed local profile
- **WHEN** its owner, collection, wrapper, identity or declaration is unsupported, truncated, duplicated or missing
- **THEN** the SDK does not publish partial attribute membership or prefix declarations

#### Scenario: Existing characteristic profiles
- **WHEN** a supported modern characteristic or the previously measured BUH characteristic is decoded
- **THEN** its ordered membership and declarations remain unchanged
