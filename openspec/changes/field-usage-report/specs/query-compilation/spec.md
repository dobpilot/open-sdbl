## ADDED Requirements

### Requirement: Report where a query reads each field
Preparation SHALL collect the fields a batch reads, each with the metadata
object it belongs to, the tabular-section name when the source is a
section, and the roles it is read in. The roles SHALL distinguish at least:
a projection of the result, a `ГДЕ` predicate, a join condition, an
`ИМЕЮЩИЕ` predicate, a grouping key, an ordering key, the argument of an
aggregate function, and a part of a computed expression.

One field read in several roles SHALL be reported in each of them, once
per role, in a stable order. A dereferenced field SHALL be reported
against the object the path ended on. The report SHALL be reachable from
the prepared query, and collecting it SHALL change no generated SQL.

#### Scenario: A field only in a predicate
- **WHEN** a statement projects one field and filters on another
- **THEN** the second is reported as read by the `ГДЕ` predicate and not
  as a projection

#### Scenario: A field in two roles
- **WHEN** a statement projects a field and also filters on it
- **THEN** the report carries it in both roles

#### Scenario: An aggregate and an expression
- **WHEN** one field is the argument of an aggregate and another is part
  of a computed expression
- **THEN** the two are reported in different roles

#### Scenario: A dereferenced field
- **WHEN** a statement reads `Т.Контрагент.ИНН`
- **THEN** the report names the counterparty catalog and its `ИНН`, not
  the document and its `Контрагент`

#### Scenario: A tabular section
- **WHEN** a statement reads a field of a tabular section
- **THEN** the report names the owning object, the section, and the field

#### Scenario: The SQL is unchanged
- **WHEN** a query is prepared and compiled
- **THEN** the statement is what it was before the report existed
