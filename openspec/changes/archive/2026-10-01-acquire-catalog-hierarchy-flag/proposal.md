# Acquire catalog hierarchy flag

## Why
A host implementing catalog Parent needs the acquired Hierarchical setting. Code properties, SQL columns and object source do not establish the configured flag.

## What Changes
Expose owner-only ConfigDescriptor.catalog_hierarchical: Option<bool> from the measured Config layout. Unknown values and layouts remain unknown. Version remains 0.9.5.

## Impact
Public metadata API; no BSL setter, saving policy, database writes or dependencies.
