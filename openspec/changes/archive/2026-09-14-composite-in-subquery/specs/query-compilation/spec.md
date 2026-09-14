## ADDED Requirements

### Requirement: A composite subquery of `В (…)`
`В (<подзапрос>)` SHALL accept a subquery whose columns are the members of
one composite value and SHALL compare the members side by side, spreading
the other side over the same members: its own member carries the value,
the discriminator its tag, every other member the zero of its type, and
each member stays `NULL` while the value is `NULL`. A subquery whose
columns are separate values SHALL keep reporting that. A dialect without a
row comparison SHALL refuse the composite form, naming the reason.

#### Scenario: A reference against a composite subquery
- **WHEN** `ГДЕ С.Ссылка В (ВЫБРАТЬ П.СоставноеПоле ИЗ Справочник.X КАК П)`
  is compiled for PostgreSQL
- **THEN** the reference is spread over the members the subquery projects
  and the members are compared side by side

#### Scenario: A subquery of unrelated columns
- **WHEN** the subquery projects two columns that are not the members of
  one value
- **THEN** the query is refused as before
