## ADDED Requirements

### Requirement: Complete measured BUH characteristic attributes
The SDK SHALL recognize homogeneous measured 2/6 wrappers with supported 27/23 bodies in characteristic own-attribute collections, preserving all native identities, names, order and qualified types. Existing modern 3/8 support SHALL remain available. Unknown, mixed, malformed, duplicate or incomplete membership SHALL remain unknown rather than partial.

#### Scenario: Native BUH chart
- **WHEN** the retained ДополнительныеРеквизитыИСведения native resource is decoded
- **THEN** all 28 own attributes match paired XML including ВидСвойства and its enumeration reference type

#### Scenario: Unsupported or incomplete membership
- **WHEN** any count, wrapper, identity or recognized child type is invalid or missing
- **THEN** the entire own-attribute collection remains unknown

#### Scenario: Existing modern collection
- **WHEN** a complete homogeneous 3/8 collection is decoded
- **THEN** its ordered complete membership remains available
