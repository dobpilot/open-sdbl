## Why

PostgreSQL Config acquisition currently materializes every row before decoding
starts, then performs CPU-heavy DEFLATE and descriptor parsing directly inside
an async task. Large information bases therefore provide no startup feedback
and cannot overlap network transfer with CPU processing.

## What Changes

- Stream Config rows with `query_raw` instead of collecting the full result.
- Decode a bounded number of resources concurrently on Tokio's blocking pool,
  providing backpressure while PostgreSQL continues delivering rows.
- Move other CPU-heavy metadata parsing and final resolution off async runtime
  workers.
- Show a TTY-only metadata progress bar on standard error, using exact Config
  resource and compressed-byte totals; keep stdout and non-TTY execution clean.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `onec-metadata`: stream and overlap PostgreSQL metadata acquisition and
  decoding with bounded memory.
- `query-repl`: report interactive metadata-loading progress without changing
  machine-readable output.

## Impact

- `open-sdbl-cli` async acquisition flow and one additional fixed SELECT-only
  Config totals query.
- Direct CLI dependencies gain `futures-util`; the root library remains free
  of production dependencies and I/O.
- No accepted metadata, stdout format, CLI syntax, or database mutation.
