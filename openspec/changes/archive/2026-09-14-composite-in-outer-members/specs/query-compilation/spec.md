## MODIFIED Requirements

### Requirement: A composite subquery of `В (…)`
`В (<подзапрос>)` SHALL accept a subquery whose columns are the members of
one composite value and SHALL compare the members side by side. A value
that is itself composite SHALL answer with its own members, and SHALL be
refused with a diagnostic naming the member it cannot answer. Every other
value SHALL be spread over the subquery's members: its own member carries
the value, the discriminator its tag, every other member the zero of its
type, and each member stays `NULL` while the value is `NULL`. The string
member SHALL be compared as text on both sides. A subquery whose columns
are separate values SHALL keep reporting that. A dialect without a row
comparison SHALL refuse the composite form, naming the reason.

#### Scenario: A composite value against a composite subquery
- **WHEN** `ГДЕ Т.Составное В (ВЫБРАТЬ Т2.Составное ИЗ Справочник.X КАК Т2)`
  is compiled
- **THEN** the outer side reads its own members rather than one of them

#### Scenario: A string against a composite subquery
- **WHEN** the outer value is a string and the subquery carries a string
  member
- **THEN** both sides compare that member as text

#### Scenario: A reference against a composite subquery
- **WHEN** the outer value is a reference
- **THEN** it is spread over the members the subquery projects

#### Scenario: A subquery of unrelated columns
- **WHEN** the subquery projects two columns that are not the members of
  one value
- **THEN** the query is refused as before
