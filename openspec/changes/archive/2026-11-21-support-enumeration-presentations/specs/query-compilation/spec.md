## ADDED Requirements

### Requirement: Compile enumeration value presentations
The compiler SHALL accept a structured enumeration presentation mapping of
reference identities to text for an enumeration target. It SHALL generate
PostgreSQL and MSSQL expressions that compare physical 1C reference bytes,
quote presentation text safely and return text. Mapping entries SHALL be
bounded, unique and validated for their enumeration owner.

#### Scenario: Present a typed enumeration reference
- **WHEN** `ПРЕДСТАВЛЕНИЕ` or `ПРЕДСТАВЛЕНИЕССЫЛКИ` presents an enumeration
  reference with a valid plan
- **THEN** generated SQL returns the text mapped to the reference identity

#### Scenario: Deferred lookup
- **WHEN** a deferred reference resolves to an enumeration
- **THEN** its presentation lookup uses the same mapping and preserves the
  lookup's existing authorization and data separation checks

#### Scenario: Source-free enumeration constant
- **WHEN** a source-free SELECT presents `ЗНАЧЕНИЕ` of an enumeration
- **THEN** compilation returns a deferred reference payload whose lookup
  applies the enumeration presentation and its access decision

#### Scenario: Unknown identity or empty reference
- **WHEN** a reference is empty, null or absent from the enumeration mapping
- **THEN** its presentation is empty text

#### Scenario: Invalid mapping
- **WHEN** a mapping contains duplicate, foreign or excessive identities or
  targets an object that is not an enumeration
- **THEN** compilation reports a presentation-plan diagnostic
