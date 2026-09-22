## ADDED Requirements

### Requirement: Answer whether a user can authenticate
`InfoBaseUser` SHALL answer whether the user has any way to log in: it
SHALL be true when standard 1C authentication is on, or when the
operating-system login is not blank once trimmed, and false otherwise.
The answer SHALL NOT depend on whether the user is shown in the login
list, on the administrative flag, on the roles the user holds, or on the
name. The administrative flag is a right, not a way in.

The rule SHALL live on `InfoBaseUser`; the database package and the
console SHALL ask it rather than restate it.

#### Scenario: Standard authentication
- **WHEN** a user has standard authentication on and no operating-system
  login
- **THEN** the user can authenticate

#### Scenario: Operating-system login
- **WHEN** a user has standard authentication off and an
  operating-system login
- **THEN** the user can authenticate

#### Scenario: Both ways
- **WHEN** a user has both
- **THEN** the user can authenticate

#### Scenario: Neither way
- **WHEN** a user has standard authentication off and a blank
  operating-system login
- **THEN** the user cannot authenticate

#### Scenario: Flags that do not decide
- **WHEN** the same user is asked with the show-in-list and
  administrative flags set and cleared
- **THEN** the answer is the same in every combination
