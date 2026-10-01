# Acquire runtime metadata for 0.9.6

## Why
The host cannot create documents or validate catalog selection without native configuration settings. Standard field authorization currently lacks a proven native mapping.

## What Changes
- Acquire complete document collections, numbering and filling settings from measured Config layouts.
- Acquire complete catalog owners and attribute indexing.
- Implement only measured standard/member permission mappings without weakening RLS.
- Release version 0.9.6.

## Impact
ConfigDescriptor and related metadata API; database field authorization. No BSL execution and no new dependencies.
