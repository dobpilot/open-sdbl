## ADDED Requirements

### Requirement: Borrow original admitted Config facts
ConfigurationMetadataCache SHALL expose parsed_resource(&self, file_name: &str) -> Option<&ParsedConfigResource> borrowing the original immutable parsed facts of a successfully admitted resource by its exact admitted filename. An unadmitted filename SHALL return None. Lookup SHALL perform no parsing, decoding, cloning, SQL or I/O and SHALL index existing facts without retaining another copy of facts or bytes. Admission limits, failed-admission behavior and resolution SHALL remain unchanged.

#### Scenario: Original native facts survive resolution
- **WHEN** a caller admits an existing native Config resource and repeatedly reads it before and after resolution
- **THEN** the borrowed facts equal independent parsing of that resource and reads refer to the same stored facts, regardless of snapshot descriptor transformations

#### Scenario: Exact and absent names
- **WHEN** a caller requests an unadmitted filename, including a differently cased filename
- **THEN** lookup returns None under the existing exact admitted-name policy

#### Scenario: Rejected admission preserves lookup
- **WHEN** a duplicate, malformed or over-budget admission fails
- **THEN** already admitted original facts remain available unchanged and a rejected new filename is absent
