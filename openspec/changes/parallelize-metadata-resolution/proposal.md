# Parallelize metadata field resolution

## Why
On the real UNF base, metadata acquisition takes about 3.73 seconds, of which final resolution takes about 2.07 seconds. Config decoding already uses bounded blocking jobs, but field resolution walks every field and can scan all objects for reference categories on one blocking worker.

## What Changes
- Resolve independent field entries concurrently on bounded scoped worker threads during snapshot finalization.
- Preserve DBNames order, descriptor source order, findings and snapshot fingerprints by joining ordered chunks.
- Keep small acquisitions single-threaded and retain the existing Tokio blocking boundary and all resource limits.
- Preserve the existing bounded Config streaming pipeline.

## Capabilities

### Modified Capabilities
- `onec-metadata`: parallel field resolution during complete snapshot construction.

## Impact
- Core metadata resolver, deterministic regression tests and live UNF performance measurements.
