## ADDED Requirements

### Requirement: Project authoritative DBNames while parsing
The metadata decoder SHALL validate the complete brace-serialized DBNames
resource and collect valid `{GUID,"Alias",Number}` entries in source order
without requiring a complete generic value tree. Memory retained during parsing
SHALL be bounded by accepted entries, decoded input, and nesting depth rather
than by every serialized scalar and list. The public generic value parser SHALL
remain available and compatible.

#### Scenario: Large nested DBNames map
- **WHEN** a DBNames resource contains many nested lists and irrelevant scalar
  values around valid entries
- **THEN** the decoder collects the valid entries without retaining generic
  nodes for the irrelevant values

#### Scenario: Malformed irrelevant branch
- **WHEN** any nested DBNames branch has an unterminated list or string even if
  it could not project to an entry
- **THEN** the complete DBNames resource is rejected with a positional
  diagnostic

#### Scenario: Entry compatibility
- **WHEN** a nested list has exactly the GUID, quoted alias, and positive number
  shape accepted by the existing projection
- **THEN** the streaming projection returns the same canonical entry in the
  same source order
