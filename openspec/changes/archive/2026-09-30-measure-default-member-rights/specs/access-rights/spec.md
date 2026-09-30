## ADDED Requirements

### Requirement: Authorize the value of a verified constant by object Read
When a field read resolves to the value of a constant and its metadata field identifier equals that constant's identifier, the database layer SHALL authorize the value through the selected user's role object `Read` grants. This mapping SHALL NOT depend on `setForAttributesByDefault`. Unresolved fields, mismatched identifiers, unread rights and unsupported member paths SHALL remain fail closed. A role explicitly denying object `Read` SHALL NOT grant the value; a different role granting object `Read` MAY grant it.

#### Scenario: The constant value is read without a member entry
- **WHEN** a role grants `Read` of a verified constant and no separate member entry exists
- **THEN** reading its value passes regardless of the default attribute flag

#### Scenario: No role grants the constant
- **WHEN** all selected roles deny the constant's object `Read`
- **THEN** reading its value fails before SQL execution

#### Scenario: Another role grants the constant
- **WHEN** one role denies and another grants object `Read` of the constant
- **THEN** its value passes the field check

#### Scenario: The identifier belongs to another object
- **WHEN** a requested field is not the resolved value of that constant
- **THEN** the constant exception does not authorize it

### Requirement: Preserve unsupported custom member authorization
Until a native member rule is implemented with its measured mapping, custom attributes and standard fields SHALL retain their existing authorization boundary. Native `View` permissions SHALL NOT be renamed into member `Read` permissions.

#### Scenario: A custom attribute has no exact member Read grant
- **WHEN** a custom attribute has no supported exact member `Read` grant
- **THEN** the field check retains its existing fail-closed result
