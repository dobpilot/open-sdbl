## MODIFIED Requirements

### Requirement: Describe result columns natively
The gateway SHALL describe and encode directly selected top-level references
with one statically known enumeration target as `text` containing the
enumeration presentation. Other references and binary data SHALL be `bytea`,
strings `text` or length-bound `varchar`, numbers `numeric` with precision and
scale, booleans `bool`, dates `timestamp`, and UUIDs `uuid`. Deferred reference
presentations SHALL be `text`. It SHALL send values in the requested text or
binary format. SQL evaluation, `DISTINCT`, UUID and type functions SHALL retain
the original reference value. Resolving enum results SHALL retain target access
decisions and separator filtering; null remains null, empty and unknown enum
references produce empty text.

#### Scenario: Enumeration reference field
- **WHEN** a client selects an enumeration-typed field in simple or extended
  protocol
- **THEN** its column is `text` and its value is the localized presentation

#### Scenario: Reference column in text format
- **WHEN** a client selects `Ссылка` of a catalog in text format
- **THEN** the column type is `bytea` and the value is `\x` followed by 32
  lower-case hex digits

#### Scenario: Number in binary format
- **WHEN** a client asks for binary results of a `numeric(15,2)` resource
- **THEN** `tokio-postgres` decodes the value to the same decimal the
  console prints

#### Scenario: Deferred reference presentation
- **WHEN** a client selects a deferred reference presentation in simple or extended protocol
- **THEN** the column has PostgreSQL type `text` and the resolved Unicode value is transmitted as UTF-8

#### Scenario: Enumeration UUID and type
- **WHEN** the same enum field is selected together with its UUID and type
- **THEN** its presentation is text while UUID and type retain their existing
  values and types
