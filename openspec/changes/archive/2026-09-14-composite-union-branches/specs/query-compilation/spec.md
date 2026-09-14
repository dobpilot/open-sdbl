## ADDED Requirements

### Requirement: Union branches of different types
A union whose branches carry one value with different shapes SHALL project
that value as a composite: every branch writes the member of its own type,
the zero of every other member and the tag of its own type, and each member
stays `NULL` while the branch value is `NULL`. A branch that already
projects the members SHALL keep their layout and the other branches SHALL
follow it. Columns that are not the members of one value SHALL keep
reporting the mismatch.

#### Scenario: A fixed reference beside a composite one
- **WHEN** a branch projecting a reference of one table is unioned with a
  branch projecting a composite reference
- **THEN** both branches project the members of the composite value

#### Scenario: A string beside a date
- **WHEN** two branches project values of different scalar types
- **THEN** each branch writes its own member, the zero of the other and the
  tag of its own type
