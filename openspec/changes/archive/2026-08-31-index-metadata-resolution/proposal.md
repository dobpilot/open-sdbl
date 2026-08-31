## Why

After Config decoding was reduced to about 1.5 seconds for the reported
information base, live console startup still exceeded two minutes. Profiling
shows that metadata resolution repeatedly scans and recases every schema and
live-catalog column for every `Fld` DBNames entry.

## What Changes

- Build schema field ownership and live field-presence indexes once before
  resolving DBNames fields.
- Index live tables by normalized physical name for object resolution and
  metadata lookup-index construction.
- Preserve field owner order, matching rules, canonical names, and all public
  resolution results.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `onec-metadata`: require metadata resolution to avoid repeated full catalog
  scans for each object and field.

## Impact

- Internal metadata resolution performance and temporary lookup memory.
- No public API, accepted metadata, dependency, or I/O change.
