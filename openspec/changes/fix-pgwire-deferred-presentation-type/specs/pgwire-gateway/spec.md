## MODIFIED Requirements

### Requirement: Describe result columns natively
The gateway SHALL describe each result column by its `ColumnKind`:
references and binary data as `bytea`, strings as `text` or length-bound
`varchar`, numbers as `numeric` with precision and scale, booleans as
`bool`, dates as `timestamp`, UUIDs as `uuid`. Deferred reference presentation columns SHALL be described and encoded as `text` after resolving the reference, regardless of the binary intermediate type. It SHALL send values in the
text or binary format the client asks for per column.

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
