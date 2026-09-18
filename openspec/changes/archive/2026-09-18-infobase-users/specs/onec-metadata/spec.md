## ADDED Requirements

### Requirement: Acquire the information-base users
The library SHALL provide, for both providers, a SELECT statement reading
`v8users` — name, description, operating-system login, the show-in-list,
standard-authentication and administrative flags, and `Data` — ordered by
name, with the flags in a form both providers answer alike.

#### Scenario: PostgreSQL
- **WHEN** the users statement is read on PostgreSQL
- **THEN** it selects from `v8users` ordered by `name`
