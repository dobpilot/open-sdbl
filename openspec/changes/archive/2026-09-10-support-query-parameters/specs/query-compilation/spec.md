## ADDED Requirements

### Requirement: Diagnose parameter binding failures
The compiler SHALL report a `Parameter` diagnostic kind when a referenced
parameter has no supplied value, when a supplied parameter is never
referenced by the source, when a list value appears outside an `В`/`IN`
operand, or when a list contains a nested list. Missing-value and misplaced
diagnostics SHALL be positioned at the offending token; the unused-parameter
diagnostic MAY be unpositioned. Preparation SHALL NOT require parameter
values.

#### Scenario: Missing value
- **WHEN** the source references `&Период` and the options carry no
  parameter of that name in any letter case
- **THEN** compilation fails with a `Parameter` diagnostic located at the
  `&Период` token

#### Scenario: Unused value
- **WHEN** the options carry a parameter the source never references
- **THEN** compilation fails with a `Parameter` diagnostic naming it

#### Scenario: Preparation without values
- **WHEN** a parameterized source is prepared
- **THEN** preparation succeeds and the presentation request is collected
