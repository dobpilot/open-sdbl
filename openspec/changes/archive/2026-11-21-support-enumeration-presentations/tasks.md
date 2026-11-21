## 1. Specification
- [x] Validate this change strictly before implementation.

## 2. Implementation
- [x] Add a bounded validated enumeration mapping to structured presentation plans and generate safe SQL for PostgreSQL and MSSQL.
- [x] Build enumeration plans from the owning Config descriptors with Russian synonym fallback.
- [x] Cover both presentation functions, ordinary and deferred lookup, access decisions, nulls, unknown identities, encoding, quoting and invalid mappings.
- [x] Present directly selected fixed-type enumeration references as text in pgwire and catalog descriptions, preserving authorization, reference operations and non-enumeration wire types.
- [x] Test the supplied accounting query and simple and extended protocol behavior for enumeration, UUID and type columns.

## 3. Verification
- [x] Run focused tests, formatting, Clippy with warnings denied, tests, rustdoc with warnings denied and strict OpenSpec validation. PostgreSQL SQL was also executed on an isolated temporary database; MSSQL was checked by generated SQL and gateway protocol tests.
