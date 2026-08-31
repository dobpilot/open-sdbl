## Why

After metadata parsing and resolution were optimized, the reported console
still spends more than 30 seconds before its first prompt. Profiling attributes
almost all remaining CPU to completion construction: candidate deduplication
scans the growing vector and queryable-field projection repeatedly scans all
resolved custom fields and reference targets.

## What Changes

- Provide a snapshot-scoped queryable-field catalog that indexes current
  custom fields by owner table and field number for the duration of one build.
- Build queryable fields once per live object while constructing completion.
- Index reference targets by physical table and deduplicate completion
  candidates with a normalized hash set while preserving insertion content.
- Preserve the complete case-insensitive completion catalog and sorted output.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `onec-metadata`: provide indexed snapshot-scoped queryable-field projection.
- `query-repl`: build metadata-aware completions without repeated full scans.

## Impact

- Queryable-field API surface and interactive console startup performance.
- No CLI syntax, completion candidate, dependency, or database I/O change.
