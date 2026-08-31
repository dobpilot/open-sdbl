## Context

`Transaction::query` returns all Config rows as a `Vec<Row>`. Each row is then
decoded synchronously in the async acquisition future. Config is tens of
thousands of independently compressed part-zero resources, so it naturally
supports a bounded fetch/decode pipeline.

## Decisions

### Stream rows into bounded blocking jobs

Use `query_raw` to obtain a row stream. Map each row into an owned file name and
compressed byte buffer, then submit parsing through `spawn_blocking`. Keep only
a small bounded number of jobs in flight and consume results in source order,
which preserves descriptor ordering and applies backpressure when CPU decoding
lags the database.

### Keep CPU work off Tokio workers

Parse DBNames and SchemaStorage and perform final metadata resolution through
`spawn_blocking` as well. PostgreSQL I/O remains on Tokio runtime workers while
pure CPU work runs on the blocking pool.

### Use exact progress totals

Run one fixed SELECT-only aggregate over the same bare-GUID, part-zero Config
predicate before opening the row stream. Render completed resources and
compressed bytes with a percentage bar. Rate-limit redraws and finish with one
stable summary line.

### Protect stdout compatibility

Render only when standard error is a terminal. Use standard error because
`metadata postgres` reserves stdout for its tabular snapshot and users may pipe
or redirect it. Non-TTY stderr emits no progress control sequences.

## Verification

Test progress rendering, streaming decode parity/error propagation, SELECT-only
query policy, TTY gating, and existing CLI output tests. Run all repository
quality gates and repeat the proxied console startup.
