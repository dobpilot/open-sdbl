## ADDED Requirements

### Requirement: Rendering a deferred presentation
A deferred presentation column that the database answers as `NULL` SHALL
stay `NULL` in the console output, because a row carrying no reference has
no presentation. A reference that no object answers SHALL keep its
unresolved marker.

#### Scenario: A row without a reference
- **WHEN** a deferred presentation column is `NULL`
- **THEN** the console prints it as `NULL`

#### Scenario: A reference no object answers
- **WHEN** the lookup finds no object for a reference
- **THEN** the console prints the unresolved marker
