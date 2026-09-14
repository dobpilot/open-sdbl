## ADDED Requirements

### Requirement: Types the server can resolve
Generated SQL SHALL carry the types the server needs to resolve it. A
value known to be `NULL` SHALL state the type it stands for wherever an
operator, a function, a grouping key or a recursive anchor is resolved by
it. `+` over strings SHALL report the string kind, because it
concatenates. A string alternative that mixes a stored string with a value
already rendered as text SHALL be rendered as text throughout, and a
comparison or a join between a stored string and a value of another string
type SHALL bring that value to the stored type, leaving the stored column
as it is. A reference path continuing past a composite hop SHALL join the
targets from the alias the composite field lives on.

#### Scenario: A date function over a null value
- **WHEN** a date function receives a value known to be `NULL`
- **THEN** the argument states the date type

#### Scenario: Concatenation reports a string
- **WHEN** `Поле + "," + Поле2` is compiled over string fields
- **THEN** the expression reports the string kind

#### Scenario: A grouping key that is null
- **WHEN** a statement groups by a value known to be `NULL`
- **THEN** the key states a type, which SQL requires of a grouping key

#### Scenario: A join between a stored string and a derived one
- **WHEN** a stored string column is joined with a column a derived source
  projects as text
- **THEN** the derived side is brought to the stored type
