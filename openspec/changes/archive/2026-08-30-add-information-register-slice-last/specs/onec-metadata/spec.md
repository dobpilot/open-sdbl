## ADDED Requirements

### Requirement: Preserve information-register field purpose
The metadata decoder SHALL classify custom information-register fields as
dimensions, resources, or attributes from the enclosing Config collection GUID
and SHALL expose that purpose on the resolved field. It SHALL NOT infer field
purpose from physical names or index layouts.

#### Scenario: Information-register dimension
- **WHEN** a descriptor is nested in the Config collection identified by
  `13134203-f60b-11d5-a3c7-0050bae0a776`
- **THEN** its resolved custom field is classified as an information-register
  dimension

#### Scenario: Unknown collection
- **WHEN** a descriptor has no recognized information-register collection
  ancestor
- **THEN** its resolved field purpose remains unknown rather than guessed

