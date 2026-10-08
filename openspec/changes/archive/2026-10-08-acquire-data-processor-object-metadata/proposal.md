# Acquire data processor object metadata

## Why
Configuration session initialization needs the real own fields and sections of data processor objects.

## What Changes
Expose optional complete ordered attribute and section membership and generated object type on ConfigDescriptor. Preserve existing qualified nested descriptors and selective acquisition.

## Impact
Public Rust descriptor fields; metadata decoder and independent native fixtures. No new dependencies, SQL behavior or application execution.
