## MODIFIED Requirements

### Requirement: Parse the authoritative physical schema
The library SHALL parse plaintext `SchemaStorage.CurrentSchema` for
`SchemaID = 0` into physical tables, columns, declared indexes, column type
tags, and reference targets. The parser SHALL preserve canonical 1C spelling
and SHALL NOT require SQL foreign keys. For an `R` declaration, an explicitly
empty target string SHALL be preserved and SHALL identify a universal
reference whose concrete type is stored in the physical `RTRef` member; it
SHALL remain distinguishable from a missing or malformed target.

#### Scenario: Reference column
- **WHEN** SchemaStorage contains a column definition with
  `{"R",...,"Reference35",...}`
- **THEN** the column exposes `Reference35` as its schema reference target

#### Scenario: Universal reference column
- **WHEN** SchemaStorage contains a valid `R` declaration with an explicitly
  empty target string
- **THEN** the parsed column retains that empty target as the universal-reference
  marker instead of reporting that the target is absent

#### Scenario: Stable schema snapshot
- **WHEN** both SchemaStorage and DBSchema exist
- **THEN** SchemaStorage remains the source used to compare with live indexes
