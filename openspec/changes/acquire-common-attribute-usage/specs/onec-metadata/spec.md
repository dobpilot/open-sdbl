## ADDED Requirements

### Requirement: Common attribute usage
The SDK SHALL acquire measured common-attribute AutoUse and complete Content entries separately from own attribute membership. Every entry SHALL retain its object GUID and explicit usage. Declared types and qualifiers SHALL use the existing descriptor representation. Unknown or incomplete forms SHALL remain None; a verified empty Content SHALL remain an empty vector.

#### Scenario: Explicitly included common attribute
- **WHEN** a measured DontUse attribute explicitly includes a catalog
- **THEN** its Content entry and declared string qualifiers are available without adding it to catalog_attributes

#### Scenario: Automatic use with exclusions
- **WHEN** a measured Use attribute explicitly excludes objects
- **THEN** AutoUse and all exclusions are preserved without guessed defaults

#### Scenario: Incomplete content
- **WHEN** a Content count, entry or usage code is unsupported
- **THEN** the entire usage is unknown rather than a partial list
