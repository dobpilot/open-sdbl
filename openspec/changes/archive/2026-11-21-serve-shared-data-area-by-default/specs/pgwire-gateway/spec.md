## MODIFIED Requirements

### Requirement: Derive the session of the authenticated user
After authentication the gateway SHALL derive the user's restrictions and
session parameters as console `\as` does and SHALL combine them with the
session parameters the operator bound to the gateway. The operator's
values SHALL win, and the client SHALL NOT set session parameters. The
gateway SHALL bind every conditional separator's condition constant to the
shared data area by default, unless the operator already bound that
condition, so a base that is not split into data areas is queryable.

#### Scenario: Separator value bound by the operator
- **WHEN** the gateway was started with a value for the data separator
  session parameter and a client queries a separated table
- **THEN** the separator predicate uses the operator's value

#### Scenario: Conditional separator defaults to the shared area
- **WHEN** a base separates a common attribute conditionally on a Config
  constant and the operator bound no condition, and a client queries a
  separated table
- **THEN** the query compiles against the shared (empty) data area instead
  of failing with `data separator requires condition constant`
