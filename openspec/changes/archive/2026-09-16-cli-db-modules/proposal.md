## Why

`db/postgres.rs` (923 lines) and `db/mssql.rs` (1 123 lines) each carry
three concerns that change for different reasons: opening and driving a
session, reading metadata through that session, and decoding the values a
provider hands back. The decoders are pure functions over provider types
and need no server, yet they sit beside connection code and share a file
with integration tests that do.

## What Changes

- Split each provider into `session.rs` (connect, query, transaction
  handling), `metadata.rs` (the `MetadataSource` implementation) and
  `cells.rs` (decoding provider values into `Cell`).
- Move their tests into `src/tests/`, one file per module.

## Capabilities

### Modified Capabilities

- `crate-architecture`: the database layer follows one feature per module.

## Impact

No behavior change. Decoding a provider value becomes testable and
reviewable without reading connection code.
