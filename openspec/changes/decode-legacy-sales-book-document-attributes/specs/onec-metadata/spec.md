## ADDED Requirements

### Requirement: Acquire measured legacy sales book document attributes
The SDK SHALL expose complete ordered own attribute membership and qualified declarations for the retained legacy sales book document profile. Unknown, malformed, unsupported, duplicate or incomplete collections SHALL remain unknown without partially publishing a prefix. Existing modern document decoding SHALL remain unchanged. Unmeasured legacy numbering, register and tabular-section metadata SHALL NOT be inferred from modern field positions.

#### Scenario: Native ordered sales book attributes
- **WHEN** the retained native legacy sales book document is acquired
- **THEN** all seven own attributes match paired XML identities, names, order and qualified declarations, including the Boolean manual-correction attribute

#### Scenario: Malformed legacy owner or member
- **WHEN** its owner, wrapper, identity, count, descriptor uniqueness or declaration is malformed or incomplete
- **THEN** complete membership and prefix declarations are not published

#### Scenario: Existing modern documents
- **WHEN** an existing supported modern document is decoded
- **THEN** its membership, declarations and other metadata remain unchanged
