## ADDED Requirements

### Requirement: Preserve measured textual legacy catalog owner comments
The decoder SHALL recognize the measured legacy reporting-directions catalog when its otherwise recognized owner header contains an empty or nonempty String comment. It SHALL preserve complete ordered own attributes and qualified declarations and the known empty native section collection. Non-string comments SHALL leave that owner profile unrecognized.

#### Scenario: Native textual owner comment
- **WHEN** the retained native reporting-directions catalog contains its nonempty textual owner comment
- **THEN** all four own UUIDs, names, order, types, references and qualifiers match paired XML
- **AND** the empty native section collection is preserved as known empty

#### Scenario: Empty String owner comment
- **WHEN** the same recognized owner has an empty String comment
- **THEN** its previously supported metadata projection remains unchanged

#### Scenario: Non-string owner comment
- **WHEN** the comment is an Atom, Null or List
- **THEN** owner memberships and declarations are not published through this profile

#### Scenario: Other header guards
- **WHEN** a child header, owner flag or other existing profile guard is invalid
- **THEN** its previous failure behavior remains unchanged
