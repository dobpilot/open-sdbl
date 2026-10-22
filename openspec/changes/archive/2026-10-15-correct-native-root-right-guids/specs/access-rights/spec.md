## MODIFIED Requirements

### Requirement: Name the standard rights
`Right` SHALL name the standard right identifiers by their platform
names — `Read`, `Insert`, `Update`, `Delete`, `View`, `Edit`,
`InputByString`, the interactive and data-history rights, the document
posting rights, `TotalsControl`, `Use`, the business-process and task
rights, the session-parameter rights `Get` and `Set`, and the
configuration rights from `Administration` to `Output` — with their
Russian synonyms where the platform has one, and SHALL carry any other
identifier as `Right::Other(guid)`.

#### Scenario: The reading right
- **WHEN** the identifier `1c87578f-9e09-4ec0-a991-5629c87b1588` is named
- **THEN** it is `Right::Read`, spelled `Чтение`

#### Scenario: An unknown identifier
- **WHEN** an identifier the table does not list is named
- **THEN** it is `Right::Other` with that identifier

#### Scenario: Independently measured configuration rights
- **WHEN** native single-right role resources are decoded
- **THEN** d8682bbb-7800-4aa0-8590-d3cb11fe2a29 is SaveUserData
- **AND** 399d7390-8d83-4a57-b4d7-c902c15b701f is ConfigurationExtensionsAdministration
- **AND** 7b8359dd-7d4e-4bcd-a61c-b4b26eae19c6 is InteractiveOpenExtDataProcessors
- **AND** eb29e198-c338-4a20-a253-be6fc3dd44d9 is InteractiveOpenExtReports
- **AND** the named grants match the retained native positive and cross-negative checks
