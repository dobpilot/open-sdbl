## ADDED Requirements

### Requirement: Preserve the measured legacy Boolean constant declaration
The SDK SHALL preserve the complete Boolean declaration of a constant in the measured legacy class14/12 profile when its native identity uniquely matches the resource descriptor. It SHALL preserve unrelated metadata and conditional references. Unknown or malformed profiles and declarations SHALL remain unknown.

#### Scenario: Whole native Boolean constant
- **GIVEN** the unchanged native BUH constant CF/XML pair and its two common attributes
- **WHEN** decoded through the public metadata parser
- **THEN** the constant has exactly one Boolean declaration without reference or qualifiers and both conditional links retain its UUID

#### Scenario: Invalid identity or declaration
- **GIVEN** a malformed shape, unknown Pattern, nil or mismatched owner identity, or missing or duplicate matching descriptor
- **WHEN** metadata is projected
- **THEN** no new declaration is published and unrelated metadata is preserved
