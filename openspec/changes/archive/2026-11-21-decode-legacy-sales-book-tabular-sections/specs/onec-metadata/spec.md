## ADDED Requirements

### Requirement: Acquire measured legacy sales book tabular sections atomically
The SDK SHALL retain complete ordered membership for the measured legacy sales book document sections and their own fields, with qualified declarations. It SHALL verify the whole newly supported section set before publishing any section membership or field declaration. Unknown, malformed, unsupported, duplicate or incomplete section metadata SHALL remain unknown without a prefix. Already acquired document own attributes and existing modern document behavior SHALL remain unchanged.

#### Scenario: Native complete section metadata
- **WHEN** the retained native legacy sales book document is acquired
- **THEN** all six section identities and all 68 ordered fields match paired XML, including all 201 qualified type entries and their original reference identities

#### Scenario: Malformed last section or field
- **WHEN** any section or field count, wrapper, identity, descriptor or declaration is malformed, missing, duplicated or unsupported
- **THEN** the SDK publishes no section prefix or partial field declarations, while retaining the already acquired seven document own attributes

#### Scenario: Existing document metadata
- **WHEN** existing modern documents or already supported legacy own attributes are acquired
- **THEN** their previous membership, declarations and unrelated metadata remain unchanged

#### Scenario: Cross-collection identity collision
- **WHEN** a section or its field aliases the document owner or an existing own attribute identity
- **THEN** section metadata remains unknown and the existing own attributes retain their exact declarations
