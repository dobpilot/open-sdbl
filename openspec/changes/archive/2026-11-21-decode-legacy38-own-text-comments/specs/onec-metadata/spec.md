## ADDED Requirements

### Requirement: Preserve measured textual comments on legacy document own attributes
The decoder SHALL preserve complete own membership and qualified declarations for the measured legacy38 document profile when own identity1/7 comments are Strings, including nonempty Strings. Other identity admission rules SHALL remain unchanged.

#### Scenario: Whole native controlled transactions document
- **GIVEN** the native CF/XML pair with nine own attributes and two nonempty own comment Strings
- **WHEN** the resource is decoded
- **THEN** all nine own attributes and the existing one section with 18 fields retain native ordered identity and all 32 qualified type rows

#### Scenario: Malformed own comment
- **GIVEN** an otherwise measured legacy38 own list containing a non-String comment
- **WHEN** the resource is decoded
- **THEN** complete own membership and new declarations remain unknown without a partial prefix

#### Scenario: Other identity callers remain strict
- **GIVEN** a nonempty comment in an owner, section or other existing strict identity profile
- **WHEN** that profile is decoded
- **THEN** its prior admission behavior remains unchanged
