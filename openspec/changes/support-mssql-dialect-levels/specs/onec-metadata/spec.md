## ADDED Requirements

### Requirement: Read the live PostgreSQL catalog on servers before 9.4
The PostgreSQL adapter SHALL read `server_version_num` inside the read-only
transaction and SHALL use a catalog statement without `LATERAL` or
`WITH ORDINALITY` when the server is older than 9.4. Both statements SHALL
return the same `(tag, table, name, detail, columns)` rows with index columns
in key order.

#### Scenario: PostgreSQL 9.2
- **WHEN** the server reports `server_version_num` below 90400
- **THEN** the adapter lists tables, columns, and ordered index keys through
  the legacy statement

#### Scenario: Modern server
- **WHEN** the server reports 9.4 or newer
- **THEN** the adapter uses the `LATERAL` statement and the resolved snapshot
  is identical to the legacy statement's result
