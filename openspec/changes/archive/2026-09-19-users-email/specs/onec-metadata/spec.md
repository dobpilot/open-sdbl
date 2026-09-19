## MODIFIED Requirements

### Requirement: Acquire the information-base users
The library SHALL provide, for both providers, a SELECT statement reading
`v8users` — name, description, operating-system login, the show-in-list,
standard-authentication and administrative flags, and `Data` — ordered by
name, with the flags in a form both providers answer alike; a statement
probing whether the table has the `Email` column, answering `1` or `0`;
and a statement reading the same columns with the e-mail after `Data`,
for a base whose table has it.

#### Scenario: PostgreSQL
- **WHEN** the users statement is read on PostgreSQL
- **THEN** it selects from `v8users` ordered by `name`

#### Scenario: E-mail column probed
- **WHEN** the probe answers `1`
- **THEN** the application reads the users with the statement carrying
  the e-mail, and with the other one otherwise
