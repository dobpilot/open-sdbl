## MODIFIED Requirements

### Requirement: Compile every query with the authenticated user's rights
By default every statement SHALL be prepared in `RestrictionMode::Restricted` with the
user's derived restrictions and SHALL pass the complete field-rights check
of the user before any SQL is generated. A target or field that cannot be
proven readable SHALL refuse the statement with SQLSTATE `42501`. When explicitly enabled, unrestricted access SHALL grant every requested target and skip field-right checks after successful authentication.

#### Scenario: Restricted catalog
- **WHEN** a user whose role restricts reading of `Справочник.Контрагенты`
  queries it without `РАЗРЕШЕННЫЕ`
- **THEN** the generated SQL carries the expanded restriction and only the
  permitted rows are returned

#### Scenario: Denied attribute
- **WHEN** the user projects an attribute no role of theirs may read
- **THEN** the client receives `42501` naming the field and the database
  receives no query


#### Scenario: Explicit unrestricted access
- **WHEN** unrestricted access is enabled and an authenticated user queries an otherwise denied object or field
- **THEN** the gateway permits access without applying role restrictions

#### Scenario: Invalid password in unrestricted mode
- **WHEN** unrestricted access is enabled and a client supplies an invalid password
- **THEN** the gateway refuses authentication with `28P01`
