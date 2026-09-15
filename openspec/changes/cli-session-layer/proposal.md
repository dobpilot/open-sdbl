## Why

The binary root of `open-sdbl-cli` is not an entry point: `main.rs` also
holds `DatabaseSession` — the facade that connects, reads metadata and
closes over both providers — together with the timeouts, the batch size
and the MSSQL transaction statement. Two responsibilities that change for
different reasons sit in one file, and the dependency runs the wrong way:
`db/mssql.rs` and `db/postgres.rs` import `CONNECTION_TIMEOUT`,
`QUERY_TIMEOUT` and `CONFIG_DECODE_BATCH_SIZE` from the crate root, so the
lower layer reaches up into the binary that drives it.

This is the first step of splitting the CLI so that one file carries one
feature.

## What Changes

- Move `DatabaseSession` and the bounded call helpers into `session.rs`.
- Move the command controller — `lex`, `metadata`, `console` — into
  `app.rs`, leaving `main.rs` as the entry point.
- Move the shared limits into `limits.rs`, so the database layer no longer
  imports them from the crate root.

## Capabilities

### Modified Capabilities

- `crate-architecture`: the CLI binary root is an entry point, and the
  layers below it do not depend on it.

## Impact

No behavior change: the same commands, diagnostics and SQL. The binary
root shrinks to its entry point, and the module graph loses its upward
edges.
