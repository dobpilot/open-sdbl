## ADDED Requirements

### Requirement: Answer a restricted compilation with decisions
The database layer SHALL answer the restriction request of a restricted
compilation with one decision per target, derived from the access a user's
roles grant for `Чтение`: a role granting the right without a restriction
SHALL yield the unrestricted decision, restrictions SHALL yield the
expanded condition, and a right no role grants SHALL yield the denied
decision. The absence of a current user, a role whose rights the base does
not carry, or any restriction-expansion error SHALL fail the whole answer.
A partially expanded set SHALL NOT be offered as sufficient for execution,
and missing data SHALL NOT be read as permission.

#### Scenario: A role grants the right outright
- **WHEN** one of the user's roles grants `Чтение` of an object without a
  restriction
- **THEN** the answer carries the unrestricted decision for that target

#### Scenario: No role grants the right
- **WHEN** no role of the user grants `Чтение` of a requested object
- **THEN** the answer carries the denied decision for that target

#### Scenario: An expansion that fails
- **WHEN** the restriction text of one target cannot be expanded
- **THEN** the whole answer fails, naming the target and the reason, and
  no decision set is returned

#### Scenario: A denial admits no row on the server
- **WHEN** a restricted query whose only target is denied runs against a
  live base
- **THEN** the server returns no row of that table, while the same query
  with the unrestricted decision returns rows

#### Scenario: No current user
- **WHEN** a restricted compilation is requested with no current user
- **THEN** the answer fails rather than returning an empty decision set
