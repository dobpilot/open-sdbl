## MODIFIED Requirements

### Requirement: Resolve data separator values from session parameters
Bound compilation SHALL resolve the use flag before the value when Config binds a use parameter. An unset bound use parameter SHALL produce the existing structural missing-session-parameter diagnostic naming that use parameter at the first source token reading a table with the separator column. A false Independent use flag SHALL reject that table before SQL execution, regardless of a supplied value, and SHALL NOT emit unrestricted SQL or request the value. A true use flag SHALL resolve the configured value binding, or the common-attribute name when no value is bound. A missing Independent value SHALL retain the structural missing-session-parameter diagnostic. Query parameters SHALL NOT supply either binding. Separators of tables not read SHALL NOT require any parameters. Preparation SHALL collect existing restriction requests without values and SHALL NOT fail on separator state. A false IndependentAndShared use flag SHALL disable its predicates and SHALL NOT require a value parameter. Without a bound use parameter, a missing IndependentAndShared value SHALL retain its typed empty-value predicate.

#### Scenario: Disabled independent separator
- **WHEN** a read table has an Independent separator with a false use parameter
- **THEN** compilation rejects the table without SQL execution and without a missing-value request

#### Scenario: Lazy use then value
- **WHEN** a read table has an Independent separator with both bound parameters missing
- **THEN** the first diagnostic requests the use parameter, and a retry with true requests the value parameter

#### Scenario: Unused table
- **WHEN** another table lacking the separator column is read
- **THEN** the missing bound use and value parameters cause no diagnostic

#### Scenario: Preparation
- **WHEN** preparation processes a separated source without parameter values
- **THEN** it collects existing requests and does not execute SQL or report missing separator parameters

#### Scenario: Session value
- **WHEN** the bound use flag is true and the value parameter is 7
- **THEN** every read table with that separator is filtered by its column equal to 7

#### Scenario: Shared default
- **WHEN** no use parameter is bound and no value is supplied for IndependentAndShared
- **THEN** its existing typed empty-value predicate remains applicable

#### Scenario: Disabled separator
- **WHEN** a bound Independent use flag is false
- **THEN** the table is rejected rather than read without a predicate

#### Scenario: Independent separator without a value
- **WHEN** the use flag is true and the bound Independent value is unset
- **THEN** compilation exposes its missing session parameter name structurally

#### Scenario: Disabled shared separator
- **WHEN** a read table has an IndependentAndShared separator with a false use flag
- **THEN** no separator predicate is emitted and no value parameter is required
